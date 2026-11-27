//! Local sync queue and application of rows pulled from the server.
//!
//! Integer primary keys stay local. Commands and pulled rows speak `public_id`.
//!
//! The `enqueue_*` functions run on the caller's transaction, so a domain
//! write and its outbox row commit together or not at all. They do nothing
//! until the hostel is linked to a cloud device: an unenrolled desk never grows
//! an outbox, and [`DbPool::link_device`] queues a full snapshot instead.

use crate::DbPool;
use protocol::{AckStatus, Entity, SyncCommand};
use sqlx::{Row, SqliteConnection};
use tracing::{error, instrument};

/// Cursor and device registration for one hostel.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct SyncState {
    /// Local hostel id.
    pub hostel_id: i64,
    /// Last pulled cursor, stored as a decimal string.
    pub cursor: String,
    /// Server device id, once enrolled.
    pub device_id: Option<String>,
    /// Receipt prefix assigned at enroll, for example `AB12CD`.
    pub device_prefix: Option<String>,
    /// Server the desk syncs with.
    pub server_url: Option<String>,
    /// Next sequence number used when `device_prefix` is set.
    pub next_receipt: i64,
}

/// One outbox row waiting to be pushed.
#[derive(Debug, Clone)]
pub struct PendingSync {
    /// Local outbox id.
    pub id: i64,
    /// Client idempotency key.
    pub idempotency_key: String,
    /// Command body.
    pub command: SyncCommand,
}

/// A pushed command staff should look at: rejected, or accepted with a warning.
#[derive(Debug, Clone, serde::Serialize)]
pub struct SyncIssue {
    /// What kind of change it was, for example `check_in`.
    pub command_type: String,
    /// Server message.
    pub message: String,
    /// True when the server rejected the change; false for a warning.
    pub rejected: bool,
    /// When the change was made on this desk.
    pub created_at: String,
}

/// Result of [`DbPool::record_acks`]: one entry per pushed outbox row.
pub type OutboxAck = (i64, AckStatus, Option<String>);

impl DbPool {
    /// Link `hostel_id` to an enrolled cloud device.
    ///
    /// With `snapshot`, and when this hostel was not already linked to
    /// `device_id`, every existing row is queued (in one transaction) so the
    /// server receives the hostel's full history. Use it for the first desk of
    /// an organization; a desk that pulled its hostel from the server passes
    /// `false`. Returns whether a snapshot was queued.
    #[instrument(skip(self))]
    pub async fn link_device(
        &self,
        hostel_id: i64,
        server_url: &str,
        device_id: &str,
        device_prefix: &str,
        snapshot: bool,
    ) -> anyhow::Result<bool> {
        let mut tx = self.inner.begin_with("BEGIN IMMEDIATE").await?;
        let previous_device: Option<String> =
            sqlx::query_scalar("SELECT device_id FROM sync_state WHERE hostel_id = ?1")
                .bind(hostel_id)
                .fetch_optional(&mut *tx)
                .await?
                .flatten();
        sqlx::query(
            r#"
            INSERT INTO sync_state (hostel_id, server_url, device_id, device_prefix)
            VALUES (?1, ?2, ?3, ?4)
            ON CONFLICT(hostel_id) DO UPDATE SET
                server_url = excluded.server_url,
                device_id = excluded.device_id,
                device_prefix = excluded.device_prefix,
                next_receipt = CASE WHEN device_prefix IS excluded.device_prefix
                                    THEN next_receipt ELSE 1 END,
                updated_at = CURRENT_TIMESTAMP
            "#,
        )
        .bind(hostel_id)
        .bind(server_url)
        .bind(device_id)
        .bind(device_prefix)
        .execute(&mut *tx)
        .await?;
        let queue = snapshot && previous_device.as_deref() != Some(device_id);
        if queue {
            sqlx::query("DELETE FROM sync_outbox WHERE hostel_id = ?1")
                .bind(hostel_id)
                .execute(&mut *tx)
                .await?;
            enqueue_snapshot(&mut tx, hostel_id).await?;
        }
        tx.commit().await?;
        Ok(queue)
    }

    /// Pending commands in the order they were recorded.
    pub async fn pending_sync(&self, hostel_id: i64) -> anyhow::Result<Vec<PendingSync>> {
        let rows = sqlx::query(
            r#"
            SELECT id, idempotency_key, payload
            FROM sync_outbox
            WHERE hostel_id = ?1 AND status = 'pending'
            ORDER BY id
            "#,
        )
        .bind(hostel_id)
        .fetch_all(&self.inner)
        .await?;
        let mut pending = Vec::with_capacity(rows.len());
        for row in rows {
            let payload: String = row.try_get("payload")?;
            pending.push(PendingSync {
                id: row.try_get("id")?,
                idempotency_key: row.try_get("idempotency_key")?,
                command: serde_json::from_str(&payload)?,
            });
        }
        Ok(pending)
    }

    /// Store push results in one transaction. A plain ack deletes the outbox
    /// row; a warning or a rejection keeps it, with the server's message, for
    /// Settings → Cloud.
    pub async fn record_acks(&self, acks: &[OutboxAck]) -> anyhow::Result<()> {
        let mut tx = self.inner.begin().await?;
        for (id, status, message) in acks {
            match (status, message) {
                (AckStatus::Acked, None) => {
                    sqlx::query("DELETE FROM sync_outbox WHERE id = ?1")
                        .bind(id)
                        .execute(&mut *tx)
                        .await?;
                }
                (status, message) => {
                    let status = match status {
                        AckStatus::Acked => "acked",
                        AckStatus::Conflict => "conflict",
                    };
                    sqlx::query("UPDATE sync_outbox SET status = ?1, error = ?2 WHERE id = ?3")
                        .bind(status)
                        .bind(message)
                        .bind(id)
                        .execute(&mut *tx)
                        .await?;
                }
            }
        }
        tx.commit().await?;
        Ok(())
    }

    /// Rejected changes and warnings, newest first.
    pub async fn sync_issues(&self, hostel_id: i64) -> anyhow::Result<Vec<SyncIssue>> {
        let rows = sqlx::query(
            r#"
            SELECT command_type, error, status, CAST(created_at AS TEXT) AS created_at
            FROM sync_outbox
            WHERE hostel_id = ?1 AND status <> 'pending' AND error IS NOT NULL
            ORDER BY id DESC
            LIMIT 50
            "#,
        )
        .bind(hostel_id)
        .fetch_all(&self.inner)
        .await?;
        rows.into_iter()
            .map(|row| {
                let status: String = row.try_get("status")?;
                Ok(SyncIssue {
                    command_type: row.try_get("command_type")?,
                    message: row.try_get("error")?,
                    rejected: status == "conflict",
                    created_at: row.try_get("created_at")?,
                })
            })
            .collect()
    }

    /// How many commands are still waiting to push.
    pub async fn pending_sync_count(&self, hostel_id: i64) -> anyhow::Result<i64> {
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM sync_outbox WHERE hostel_id = ?1 AND status = 'pending'",
        )
        .bind(hostel_id)
        .fetch_one(&self.inner)
        .await?;
        Ok(count)
    }

    /// The earliest hostel, which is the one this desktop syncs.
    pub async fn primary_hostel_id(&self) -> anyhow::Result<Option<i64>> {
        let id = sqlx::query_scalar("SELECT id FROM hostels ORDER BY id LIMIT 1")
            .fetch_optional(&self.inner)
            .await?;
        Ok(id)
    }

    /// Stable id for a hostel.
    pub async fn hostel_public_id(&self, hostel_id: i64) -> anyhow::Result<Option<String>> {
        let id = sqlx::query_scalar("SELECT public_id FROM hostels WHERE id = ?1")
            .bind(hostel_id)
            .fetch_optional(&self.inner)
            .await?;
        Ok(id)
    }

    /// True when any hostel on this desk is linked to a cloud device.
    pub async fn is_enrolled(&self) -> anyhow::Result<bool> {
        let linked: i64 = sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM sync_state WHERE device_id IS NOT NULL)",
        )
        .fetch_one(&self.inner)
        .await?;
        Ok(linked != 0)
    }

    /// Read sync registration for a hostel.
    pub async fn sync_state(&self, hostel_id: i64) -> anyhow::Result<Option<SyncState>> {
        let row = sqlx::query_as::<_, SyncState>(
            r#"
            SELECT hostel_id, cursor, device_id, device_prefix, server_url, next_receipt
            FROM sync_state WHERE hostel_id = ?1
            "#,
        )
        .bind(hostel_id)
        .fetch_optional(&self.inner)
        .await?;
        Ok(row)
    }

    /// Apply one pulled page and remember `cursor`, in one transaction, so a
    /// crash can never leave rows applied without the cursor that covers them.
    ///
    /// Returns the local id of `hostel_public_id`, which exists afterwards
    /// whenever the page (or an earlier one) carried the hostel.
    #[instrument(skip(self, entities))]
    pub async fn apply_remote_page(
        &self,
        hostel_public_id: &str,
        entities: &[Entity],
        cursor: i64,
    ) -> anyhow::Result<Option<i64>> {
        let mut ordered: Vec<&Entity> = entities.iter().collect();
        // Stable: rows of one kind keep server order, so the latest wins.
        ordered.sort_by_key(|entity| entity_rank(entity));
        let mut tx = self.inner.begin_with("BEGIN IMMEDIATE").await?;
        for entity in ordered {
            apply_one(&mut tx, entity).await?;
        }
        sqlx::query(
            r#"
            UPDATE rooms
            SET status = CASE
                WHEN (
                    SELECT COUNT(*) FROM allocations
                    WHERE allocations.room_id = rooms.id AND allocations.status = 'active'
                ) > 0 THEN 'occupied'
                ELSE 'vacant'
            END
            WHERE status != 'maintenance'
            "#,
        )
        .execute(&mut *tx)
        .await?;
        let hostel_id: Option<i64> =
            sqlx::query_scalar("SELECT id FROM hostels WHERE public_id = ?1")
                .bind(hostel_public_id)
                .fetch_optional(&mut *tx)
                .await?;
        if let Some(hostel_id) = hostel_id {
            sqlx::query(
                r#"
                INSERT INTO sync_state (hostel_id, cursor) VALUES (?1, ?2)
                ON CONFLICT(hostel_id) DO UPDATE SET
                    cursor = excluded.cursor,
                    updated_at = CURRENT_TIMESTAMP
                "#,
            )
            .bind(hostel_id)
            .bind(cursor.to_string())
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(hostel_id)
    }
}

/// Queue every existing row of `hostel_id` as creates, in an order the server
/// can apply: hostel, preferences, rooms, residents without a stay, each stay
/// (check-in, then check-out when it ended), then payments.
///
/// Local revisions reset to 1 to match the rows the server will create.
async fn enqueue_snapshot(conn: &mut SqliteConnection, hostel_id: i64) -> anyhow::Result<()> {
    for sql in [
        "UPDATE hostels SET revision = 1 WHERE id = ?1",
        "UPDATE rooms SET revision = 1 WHERE hostel_id = ?1",
        "UPDATE residents SET revision = 1 WHERE hostel_id = ?1",
        "UPDATE allocations SET revision = 1 WHERE hostel_id = ?1",
        "UPDATE app_preferences SET revision = 1 WHERE hostel_id = ?1",
    ] {
        sqlx::query(sql).bind(hostel_id).execute(&mut *conn).await?;
    }
    enqueue_hostel(conn, hostel_id).await?;
    let has_prefs: Option<i64> =
        sqlx::query_scalar("SELECT hostel_id FROM app_preferences WHERE hostel_id = ?1")
            .bind(hostel_id)
            .fetch_optional(&mut *conn)
            .await?;
    if has_prefs.is_some() {
        enqueue_preferences(conn, hostel_id).await?;
    }
    let rooms: Vec<i64> =
        sqlx::query_scalar("SELECT id FROM rooms WHERE hostel_id = ?1 ORDER BY id")
            .bind(hostel_id)
            .fetch_all(&mut *conn)
            .await?;
    for room_id in rooms {
        enqueue_room(conn, hostel_id, room_id).await?;
    }
    let lone_residents: Vec<i64> = sqlx::query_scalar(
        r#"
        SELECT id FROM residents
        WHERE hostel_id = ?1
          AND NOT EXISTS (SELECT 1 FROM allocations a WHERE a.resident_id = residents.id)
        ORDER BY id
        "#,
    )
    .bind(hostel_id)
    .fetch_all(&mut *conn)
    .await?;
    for resident_id in lone_residents {
        enqueue_resident(conn, hostel_id, resident_id).await?;
    }
    let stays: Vec<(i64, String)> =
        sqlx::query_as("SELECT id, status FROM allocations WHERE hostel_id = ?1 ORDER BY id")
            .bind(hostel_id)
            .fetch_all(&mut *conn)
            .await?;
    for (allocation_id, status) in stays {
        enqueue_check_in(conn, hostel_id, allocation_id).await?;
        if status == "checked_out" {
            enqueue_check_out(conn, hostel_id, allocation_id).await?;
        }
    }
    let payments: Vec<i64> =
        sqlx::query_scalar("SELECT id FROM payments WHERE hostel_id = ?1 ORDER BY id")
            .bind(hostel_id)
            .fetch_all(&mut *conn)
            .await?;
    for payment_id in payments {
        enqueue_payment(conn, hostel_id, payment_id).await?;
    }
    Ok(())
}

async fn enrolled(conn: &mut SqliteConnection, hostel_id: i64) -> anyhow::Result<bool> {
    let linked: i64 = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM sync_state WHERE hostel_id = ?1 AND device_id IS NOT NULL)",
    )
    .bind(hostel_id)
    .fetch_one(&mut *conn)
    .await?;
    Ok(linked != 0)
}

async fn enqueue(
    conn: &mut SqliteConnection,
    hostel_id: i64,
    command: &SyncCommand,
) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        INSERT INTO sync_outbox (hostel_id, idempotency_key, command_type, payload)
        VALUES (?1, ?2, ?3, ?4)
        "#,
    )
    .bind(hostel_id)
    .bind(new_idempotency_key()?)
    .bind(command_type(command))
    .bind(serde_json::to_string(command)?)
    .execute(&mut *conn)
    .await
    .inspect_err(|e| error!(error = %e, hostel_id, "database error enqueueing sync command"))?;
    Ok(())
}

async fn hostel_public_id(conn: &mut SqliteConnection, hostel_id: i64) -> anyhow::Result<String> {
    sqlx::query_scalar("SELECT public_id FROM hostels WHERE id = ?1")
        .bind(hostel_id)
        .fetch_optional(&mut *conn)
        .await?
        .ok_or_else(|| anyhow::anyhow!("hostel not found"))
}

/// Queue the current hostel profile, when the hostel is linked.
pub(crate) async fn enqueue_hostel(
    conn: &mut SqliteConnection,
    hostel_id: i64,
) -> anyhow::Result<()> {
    if !enrolled(conn, hostel_id).await? {
        return Ok(());
    }
    let row = sqlx::query(
        "SELECT public_id, revision, name, address, phone, currency FROM hostels WHERE id = ?1",
    )
    .bind(hostel_id)
    .fetch_optional(&mut *conn)
    .await?
    .ok_or_else(|| anyhow::anyhow!("hostel not found"))?;
    let command = SyncCommand::UpsertHostel {
        public_id: row.try_get("public_id")?,
        base_revision: base_revision(row.try_get("revision")?),
        name: row.try_get("name")?,
        address: row.try_get("address")?,
        phone: row.try_get("phone")?,
        currency: row.try_get("currency")?,
    };
    enqueue(conn, hostel_id, &command).await
}

/// Queue the current room row, when the hostel is linked.
pub(crate) async fn enqueue_room(
    conn: &mut SqliteConnection,
    hostel_id: i64,
    room_id: i64,
) -> anyhow::Result<()> {
    if !enrolled(conn, hostel_id).await? {
        return Ok(());
    }
    let hostel_public_id = hostel_public_id(conn, hostel_id).await?;
    let row = sqlx::query(
        r#"
        SELECT public_id, revision, room_number, room_type, capacity, floor,
               semester_rate, recess_rate, status
        FROM rooms WHERE id = ?1 AND hostel_id = ?2
        "#,
    )
    .bind(room_id)
    .bind(hostel_id)
    .fetch_optional(&mut *conn)
    .await?
    .ok_or_else(|| anyhow::anyhow!("room not found"))?;
    let command = SyncCommand::UpsertRoom {
        public_id: row.try_get("public_id")?,
        hostel_public_id,
        base_revision: base_revision(row.try_get("revision")?),
        room_number: row.try_get("room_number")?,
        room_type: row.try_get("room_type")?,
        capacity: row.try_get("capacity")?,
        floor: row.try_get("floor")?,
        semester_rate: row.try_get("semester_rate")?,
        recess_rate: row.try_get("recess_rate")?,
        status: row.try_get("status")?,
    };
    enqueue(conn, hostel_id, &command).await
}

/// Queue the current resident row, when the hostel is linked.
pub(crate) async fn enqueue_resident(
    conn: &mut SqliteConnection,
    hostel_id: i64,
    resident_id: i64,
) -> anyhow::Result<()> {
    if !enrolled(conn, hostel_id).await? {
        return Ok(());
    }
    let hostel_public_id = hostel_public_id(conn, hostel_id).await?;
    let row = sqlx::query(
        r#"
        SELECT public_id, revision, full_name, phone, email, id_number, next_of_kin, next_of_kin_phone
        FROM residents WHERE id = ?1 AND hostel_id = ?2
        "#,
    )
    .bind(resident_id)
    .bind(hostel_id)
    .fetch_optional(&mut *conn)
    .await?
    .ok_or_else(|| anyhow::anyhow!("resident not found"))?;
    let command = SyncCommand::UpsertResident {
        public_id: row.try_get("public_id")?,
        hostel_public_id,
        base_revision: base_revision(row.try_get("revision")?),
        full_name: row.try_get("full_name")?,
        phone: row.try_get("phone")?,
        email: row.try_get("email")?,
        id_number: row.try_get("id_number")?,
        next_of_kin: row.try_get("next_of_kin")?,
        next_of_kin_phone: row.try_get("next_of_kin_phone")?,
    };
    enqueue(conn, hostel_id, &command).await
}

/// Queue the current preferences row, when the hostel is linked.
pub(crate) async fn enqueue_preferences(
    conn: &mut SqliteConnection,
    hostel_id: i64,
) -> anyhow::Result<()> {
    if !enrolled(conn, hostel_id).await? {
        return Ok(());
    }
    let hostel_public_id = hostel_public_id(conn, hostel_id).await?;
    let row = sqlx::query(
        "SELECT revision, date_format, language, receipt_footer FROM app_preferences WHERE hostel_id = ?1",
    )
    .bind(hostel_id)
    .fetch_optional(&mut *conn)
    .await?
    .ok_or_else(|| anyhow::anyhow!("preferences not found"))?;
    let command = SyncCommand::UpsertPreferences {
        hostel_public_id,
        base_revision: base_revision(row.try_get("revision")?),
        date_format: row.try_get("date_format")?,
        language: row.try_get("language")?,
        receipt_footer: row.try_get("receipt_footer")?,
    };
    enqueue(conn, hostel_id, &command).await
}

/// Queue a stored check-in, when the hostel is linked.
pub(crate) async fn enqueue_check_in(
    conn: &mut SqliteConnection,
    hostel_id: i64,
    allocation_id: i64,
) -> anyhow::Result<()> {
    if !enrolled(conn, hostel_id).await? {
        return Ok(());
    }
    let row = sqlx::query(
        r#"
        SELECT a.public_id AS allocation_public_id,
               h.public_id AS hostel_public_id,
               res.public_id AS resident_public_id,
               rooms.public_id AS room_public_id,
               res.full_name AS full_name,
               res.phone AS phone,
               res.email AS email,
               res.id_number AS id_number,
               res.next_of_kin AS next_of_kin,
               res.next_of_kin_phone AS next_of_kin_phone,
               a.rate_agreed AS rate_agreed,
               a.term AS term,
               a.academic_year AS academic_year,
               strftime('%Y-%m-%d %H:%M:%S', a.created_at) AS checked_in_at
        FROM allocations a
        JOIN hostels h ON h.id = a.hostel_id
        JOIN residents res ON res.id = a.resident_id
        JOIN rooms ON rooms.id = a.room_id
        WHERE a.id = ?1 AND a.hostel_id = ?2
        "#,
    )
    .bind(allocation_id)
    .bind(hostel_id)
    .fetch_optional(&mut *conn)
    .await?
    .ok_or_else(|| anyhow::anyhow!("allocation not found"))?;
    let command = SyncCommand::CheckIn {
        allocation_public_id: row.try_get("allocation_public_id")?,
        hostel_public_id: row.try_get("hostel_public_id")?,
        resident_public_id: row.try_get("resident_public_id")?,
        room_public_id: row.try_get("room_public_id")?,
        full_name: row.try_get("full_name")?,
        phone: row.try_get("phone")?,
        email: row.try_get("email")?,
        id_number: row.try_get("id_number")?,
        next_of_kin: row.try_get("next_of_kin")?,
        next_of_kin_phone: row.try_get("next_of_kin_phone")?,
        rate_agreed: row.try_get("rate_agreed")?,
        term: row.try_get("term")?,
        academic_year: row.try_get("academic_year")?,
        checked_in_at: row.try_get("checked_in_at")?,
    };
    enqueue(conn, hostel_id, &command).await
}

/// Queue a stored check-out, when the hostel is linked.
pub(crate) async fn enqueue_check_out(
    conn: &mut SqliteConnection,
    hostel_id: i64,
    allocation_id: i64,
) -> anyhow::Result<()> {
    if !enrolled(conn, hostel_id).await? {
        return Ok(());
    }
    let row = sqlx::query(
        r#"
        SELECT a.public_id AS allocation_public_id, h.public_id AS hostel_public_id,
               strftime('%Y-%m-%d %H:%M:%S', a.updated_at) AS checked_out_at
        FROM allocations a JOIN hostels h ON h.id = a.hostel_id
        WHERE a.id = ?1 AND a.hostel_id = ?2
        "#,
    )
    .bind(allocation_id)
    .bind(hostel_id)
    .fetch_optional(&mut *conn)
    .await?
    .ok_or_else(|| anyhow::anyhow!("allocation not found"))?;
    let command = SyncCommand::CheckOut {
        hostel_public_id: row.try_get("hostel_public_id")?,
        allocation_public_id: row.try_get("allocation_public_id")?,
        checked_out_at: row.try_get("checked_out_at")?,
    };
    enqueue(conn, hostel_id, &command).await
}

/// Queue a payment and its printed receipt number, when the hostel is linked.
pub(crate) async fn enqueue_payment(
    conn: &mut SqliteConnection,
    hostel_id: i64,
    payment_id: i64,
) -> anyhow::Result<()> {
    if !enrolled(conn, hostel_id).await? {
        return Ok(());
    }
    let row = sqlx::query(
        r#"
        SELECT p.public_id AS payment_public_id,
               rc.public_id AS receipt_public_id,
               rc.receipt_number AS receipt_number,
               h.public_id AS hostel_public_id,
               a.public_id AS allocation_public_id,
               res.public_id AS resident_public_id,
               p.amount AS amount,
               p.payment_method AS payment_method,
               strftime('%Y-%m-%dT%H:%M:%SZ', p.payment_date) AS payment_date,
               p.notes AS notes
        FROM payments p
        JOIN receipts rc ON rc.payment_id = p.id
        JOIN hostels h ON h.id = p.hostel_id
        JOIN allocations a ON a.id = p.allocation_id
        JOIN residents res ON res.id = p.resident_id
        WHERE p.id = ?1 AND p.hostel_id = ?2
        "#,
    )
    .bind(payment_id)
    .bind(hostel_id)
    .fetch_optional(&mut *conn)
    .await?
    .ok_or_else(|| anyhow::anyhow!("payment not found"))?;
    let command = SyncCommand::RecordPayment {
        payment_public_id: row.try_get("payment_public_id")?,
        receipt_public_id: row.try_get("receipt_public_id")?,
        hostel_public_id: row.try_get("hostel_public_id")?,
        allocation_public_id: row.try_get("allocation_public_id")?,
        resident_public_id: row.try_get("resident_public_id")?,
        amount: row.try_get("amount")?,
        payment_method: row.try_get("payment_method")?,
        receipt_number: row.try_get("receipt_number")?,
        payment_date: row.try_get("payment_date")?,
        notes: row.try_get("notes")?,
    };
    enqueue(conn, hostel_id, &command).await
}

fn entity_rank(entity: &Entity) -> u8 {
    match entity {
        Entity::Hostel(_) => 0,
        Entity::Room(_) => 1,
        Entity::Resident(_) => 2,
        Entity::Preferences(_) => 3,
        Entity::Allocation(_) => 4,
        Entity::Payment(_) => 5,
        Entity::Receipt(_) => 6,
    }
}

fn base_revision(current: i64) -> i64 {
    if current <= 1 {
        0
    } else {
        current - 1
    }
}

fn command_type(command: &SyncCommand) -> &'static str {
    match command {
        SyncCommand::UpsertHostel { .. } => "upsert_hostel",
        SyncCommand::UpsertRoom { .. } => "upsert_room",
        SyncCommand::UpsertResident { .. } => "upsert_resident",
        SyncCommand::UpsertPreferences { .. } => "upsert_preferences",
        SyncCommand::CheckIn { .. } => "check_in",
        SyncCommand::CheckOut { .. } => "check_out",
        SyncCommand::RecordPayment { .. } => "record_payment",
    }
}

fn new_idempotency_key() -> anyhow::Result<String> {
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes)?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

async fn apply_one(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    entity: &Entity,
) -> anyhow::Result<()> {
    match entity {
        Entity::Hostel(hostel) => apply_hostel(tx, hostel).await,
        Entity::Room(room) => apply_room(tx, room).await,
        Entity::Resident(resident) => apply_resident(tx, resident).await,
        Entity::Allocation(allocation) => apply_allocation(tx, allocation).await,
        Entity::Payment(payment) => apply_payment(tx, payment).await,
        Entity::Receipt(receipt) => apply_receipt(tx, receipt).await,
        Entity::Preferences(prefs) => apply_preferences(tx, prefs).await,
    }
}

/// Update the hostel with this public id, or create it on an empty desk.
/// A desk that already holds a different hostel refuses: the desk only ever
/// joins an organization from a fresh install.
async fn apply_hostel(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    hostel: &protocol::HostelSnap,
) -> anyhow::Result<()> {
    let updated = sqlx::query(
        r#"
        UPDATE hostels
        SET name = ?1, address = ?2, phone = ?3, currency = ?4, revision = ?5
        WHERE public_id = ?6
        "#,
    )
    .bind(&hostel.name)
    .bind(&hostel.address)
    .bind(&hostel.phone)
    .bind(&hostel.currency)
    .bind(hostel.revision)
    .bind(&hostel.public_id)
    .execute(&mut **tx)
    .await?
    .rows_affected();
    if updated > 0 {
        return Ok(());
    }
    let local: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM hostels")
        .fetch_one(&mut **tx)
        .await?;
    if local > 0 {
        anyhow::bail!("the cloud hostel is not the hostel on this computer");
    }
    sqlx::query(
        r#"
        INSERT INTO hostels (name, address, phone, currency, public_id, revision)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6)
        "#,
    )
    .bind(&hostel.name)
    .bind(&hostel.address)
    .bind(&hostel.phone)
    .bind(&hostel.currency)
    .bind(&hostel.public_id)
    .bind(hostel.revision)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

async fn apply_room(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    room: &protocol::RoomSnap,
) -> anyhow::Result<()> {
    let hostel_id = require_local_id(tx, "hostels", &room.hostel_public_id).await?;
    let existing: Option<i64> = sqlx::query_scalar("SELECT id FROM rooms WHERE public_id = ?1")
        .bind(&room.public_id)
        .fetch_optional(&mut **tx)
        .await?;
    // Status is taken as sent; apply_remote_page then recounts occupancy for
    // every room not under maintenance.
    if let Some(id) = existing {
        sqlx::query(
            r#"
            UPDATE rooms
            SET room_number = ?1, room_type = ?2, capacity = ?3, floor = ?4,
                semester_rate = ?5, recess_rate = ?6, revision = ?7, status = ?8
            WHERE id = ?9
            "#,
        )
        .bind(&room.room_number)
        .bind(&room.room_type)
        .bind(room.capacity)
        .bind(room.floor)
        .bind(&room.semester_rate)
        .bind(&room.recess_rate)
        .bind(room.revision)
        .bind(&room.status)
        .bind(id)
        .execute(&mut **tx)
        .await?;
        return Ok(());
    }
    sqlx::query(
        r#"
        INSERT INTO rooms (
            hostel_id, room_number, room_type, capacity, floor,
            semester_rate, recess_rate, status, public_id, revision
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
        "#,
    )
    .bind(hostel_id)
    .bind(&room.room_number)
    .bind(&room.room_type)
    .bind(room.capacity)
    .bind(room.floor)
    .bind(&room.semester_rate)
    .bind(&room.recess_rate)
    .bind(&room.status)
    .bind(&room.public_id)
    .bind(room.revision)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

async fn apply_resident(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    resident: &protocol::ResidentSnap,
) -> anyhow::Result<()> {
    let hostel_id = require_local_id(tx, "hostels", &resident.hostel_public_id).await?;
    let existing: Option<i64> = sqlx::query_scalar("SELECT id FROM residents WHERE public_id = ?1")
        .bind(&resident.public_id)
        .fetch_optional(&mut **tx)
        .await?;
    if let Some(id) = existing {
        sqlx::query(
            r#"
            UPDATE residents
            SET full_name = ?1, phone = ?2, email = ?3, id_number = ?4,
                next_of_kin = ?5, next_of_kin_phone = ?6, revision = ?7, hostel_id = ?8
            WHERE id = ?9
            "#,
        )
        .bind(&resident.full_name)
        .bind(&resident.phone)
        .bind(&resident.email)
        .bind(&resident.id_number)
        .bind(&resident.next_of_kin)
        .bind(&resident.next_of_kin_phone)
        .bind(resident.revision)
        .bind(hostel_id)
        .bind(id)
        .execute(&mut **tx)
        .await?;
        return Ok(());
    }
    sqlx::query(
        r#"
        INSERT INTO residents (
            hostel_id, full_name, phone, email, id_number, next_of_kin, next_of_kin_phone,
            public_id, revision
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
        "#,
    )
    .bind(hostel_id)
    .bind(&resident.full_name)
    .bind(&resident.phone)
    .bind(&resident.email)
    .bind(&resident.id_number)
    .bind(&resident.next_of_kin)
    .bind(&resident.next_of_kin_phone)
    .bind(&resident.public_id)
    .bind(resident.revision)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

async fn apply_allocation(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    allocation: &protocol::AllocationSnap,
) -> anyhow::Result<()> {
    let hostel_id = require_local_id(tx, "hostels", &allocation.hostel_public_id).await?;
    let resident_id = require_local_id(tx, "residents", &allocation.resident_public_id).await?;
    let room_id = require_local_id(tx, "rooms", &allocation.room_public_id).await?;
    let updated_at = allocation
        .checked_out_at
        .as_deref()
        .unwrap_or(&allocation.checked_in_at);
    let existing: Option<i64> =
        sqlx::query_scalar("SELECT id FROM allocations WHERE public_id = ?1")
            .bind(&allocation.public_id)
            .fetch_optional(&mut **tx)
            .await?;
    if let Some(id) = existing {
        sqlx::query(
            r#"
            UPDATE allocations
            SET status = ?1, rate_agreed = ?2, term = ?3, academic_year = ?4, revision = ?5,
                updated_at = ?6
            WHERE id = ?7
            "#,
        )
        .bind(&allocation.status)
        .bind(&allocation.rate_agreed)
        .bind(&allocation.term)
        .bind(&allocation.academic_year)
        .bind(allocation.revision)
        .bind(updated_at)
        .bind(id)
        .execute(&mut **tx)
        .await?;
        return Ok(());
    }
    sqlx::query(
        r#"
        INSERT INTO allocations (
            hostel_id, resident_id, room_id, status, rate_agreed, term, academic_year,
            public_id, revision, created_at, updated_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
        "#,
    )
    .bind(hostel_id)
    .bind(resident_id)
    .bind(room_id)
    .bind(&allocation.status)
    .bind(&allocation.rate_agreed)
    .bind(&allocation.term)
    .bind(&allocation.academic_year)
    .bind(&allocation.public_id)
    .bind(allocation.revision)
    .bind(&allocation.checked_in_at)
    .bind(updated_at)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

async fn apply_payment(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    payment: &protocol::PaymentSnap,
) -> anyhow::Result<()> {
    let existing: Option<i64> = sqlx::query_scalar("SELECT id FROM payments WHERE public_id = ?1")
        .bind(&payment.public_id)
        .fetch_optional(&mut **tx)
        .await?;
    if existing.is_some() {
        return Ok(());
    }
    protocol::validate_money("amount", &payment.amount)
        .map_err(|e| anyhow::anyhow!("pulled payment {}: {e}", payment.public_id))?;
    let hostel_id = require_local_id(tx, "hostels", &payment.hostel_public_id).await?;
    let allocation_id = require_local_id(tx, "allocations", &payment.allocation_public_id).await?;
    let resident_id = require_local_id(tx, "residents", &payment.resident_public_id).await?;
    sqlx::query(
        r#"
        INSERT INTO payments (
            hostel_id, allocation_id, resident_id, amount, payment_method, payment_date, notes,
            public_id
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
        "#,
    )
    .bind(hostel_id)
    .bind(allocation_id)
    .bind(resident_id)
    .bind(&payment.amount)
    .bind(&payment.payment_method)
    .bind(&payment.payment_date)
    .bind(&payment.notes)
    .bind(&payment.public_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

async fn apply_receipt(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    receipt: &protocol::ReceiptSnap,
) -> anyhow::Result<()> {
    let existing: Option<i64> = sqlx::query_scalar("SELECT id FROM receipts WHERE public_id = ?1")
        .bind(&receipt.public_id)
        .fetch_optional(&mut **tx)
        .await?;
    if existing.is_some() {
        return Ok(());
    }
    let hostel_id = require_local_id(tx, "hostels", &receipt.hostel_public_id).await?;
    let payment_id = require_local_id(tx, "payments", &receipt.payment_public_id).await?;
    sqlx::query(
        r#"
        INSERT INTO receipts (hostel_id, payment_id, receipt_number, public_id)
        VALUES (?1, ?2, ?3, ?4)
        "#,
    )
    .bind(hostel_id)
    .bind(payment_id)
    .bind(&receipt.receipt_number)
    .bind(&receipt.public_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

async fn apply_preferences(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    prefs: &protocol::PrefsSnap,
) -> anyhow::Result<()> {
    let hostel_id = require_local_id(tx, "hostels", &prefs.hostel_public_id).await?;
    sqlx::query(
        r#"
        INSERT INTO app_preferences (hostel_id, date_format, language, receipt_footer, revision)
        VALUES (?1, ?2, ?3, ?4, ?5)
        ON CONFLICT(hostel_id) DO UPDATE SET
            date_format = excluded.date_format,
            language = excluded.language,
            receipt_footer = excluded.receipt_footer,
            revision = excluded.revision,
            updated_at = CURRENT_TIMESTAMP
        "#,
    )
    .bind(hostel_id)
    .bind(&prefs.date_format)
    .bind(&prefs.language)
    .bind(&prefs.receipt_footer)
    .bind(prefs.revision)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

async fn require_local_id(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    table: &str,
    public_id: &str,
) -> anyhow::Result<i64> {
    let sql = match table {
        "hostels" => "SELECT id FROM hostels WHERE public_id = ?1",
        "rooms" => "SELECT id FROM rooms WHERE public_id = ?1",
        "residents" => "SELECT id FROM residents WHERE public_id = ?1",
        "allocations" => "SELECT id FROM allocations WHERE public_id = ?1",
        "payments" => "SELECT id FROM payments WHERE public_id = ?1",
        _ => anyhow::bail!("unknown sync table"),
    };
    let id: Option<i64> = sqlx::query_scalar(sql)
        .bind(public_id)
        .fetch_optional(&mut **tx)
        .await?;
    id.ok_or_else(|| anyhow::anyhow!("{table} {public_id} is not in the local cache"))
}

#[cfg(test)]
mod tests {
    use crate::model::*;
    use crate::test_support::*;
    use protocol::{Entity, RoomSnap, SyncCommand};

    async fn link(pool: &crate::DbPool, hostel_id: i64) {
        pool.link_device(hostel_id, "https://sync.test", "dev-1", "AB12CD", true)
            .await
            .expect("link");
    }

    fn payment(hostel_id: i64, allocation_id: i64, resident_id: i64, amount: &str) -> NewPayment {
        NewPayment {
            hostel_id,
            allocation_id,
            resident_id,
            amount: amount.to_string(),
            payment_method: "cash".to_string(),
            period_start: None,
            period_end: None,
            notes: None,
        }
    }

    #[tokio::test]
    async fn prefixed_receipt_uses_device_sequence() {
        let pool = test_pool().await;
        let hostel = seed_hostel(&pool).await;
        let room_id = seed_room(&pool, hostel.id, "1", "single").await;
        let resident_id = seed_resident(&pool, "Ada").await;
        let allocation_id = seed_active_allocation(&pool, hostel.id, resident_id, room_id).await;
        link(&pool, hostel.id).await;
        sqlx::query("UPDATE sync_state SET next_receipt = 7 WHERE hostel_id = ?1")
            .bind(hostel.id)
            .execute(&pool.inner)
            .await
            .expect("sequence");

        let (_, first) = pool
            .record_payment_and_receipt(payment(hostel.id, allocation_id, resident_id, "10"))
            .await
            .expect("first receipt");
        assert_eq!(first, "AB12CD-0007");
        let (_, second) = pool
            .record_payment_and_receipt(payment(hostel.id, allocation_id, resident_id, "5"))
            .await
            .expect("second receipt");
        assert_eq!(second, "AB12CD-0008");
    }

    #[tokio::test]
    async fn unenrolled_desk_keeps_no_outbox_and_linking_snapshots_history() {
        let pool = test_pool().await;
        let hostel = seed_hostel(&pool).await;
        let room_id = seed_room(&pool, hostel.id, "1", "single").await;
        let resident_id = seed_resident(&pool, "Ada").await;
        let allocation_id = seed_active_allocation(&pool, hostel.id, resident_id, room_id).await;
        pool.record_payment_and_receipt(payment(hostel.id, allocation_id, resident_id, "10"))
            .await
            .expect("pay");
        assert_eq!(pool.pending_sync_count(hostel.id).await.unwrap(), 0);

        assert!(pool
            .link_device(hostel.id, "https://sync.test", "dev-1", "AB12CD", true)
            .await
            .unwrap());
        let kinds: Vec<&'static str> = pool
            .pending_sync(hostel.id)
            .await
            .unwrap()
            .iter()
            .map(|row| match row.command {
                SyncCommand::UpsertHostel { .. } => "hostel",
                SyncCommand::UpsertPreferences { .. } => "prefs",
                SyncCommand::UpsertRoom { .. } => "room",
                SyncCommand::UpsertResident { .. } => "resident",
                SyncCommand::CheckIn { .. } => "check_in",
                SyncCommand::CheckOut { .. } => "check_out",
                SyncCommand::RecordPayment { .. } => "payment",
            })
            .collect();
        assert_eq!(kinds.first(), Some(&"hostel"));
        assert!(
            kinds.ends_with(&["room", "check_in", "payment"]),
            "{kinds:?}"
        );
        // Linking the same device again does not queue a second copy.
        let before = pool.pending_sync_count(hostel.id).await.unwrap();
        assert!(!pool
            .link_device(hostel.id, "https://sync.test", "dev-1", "AB12CD", true)
            .await
            .unwrap());
        assert_eq!(pool.pending_sync_count(hostel.id).await.unwrap(), before);
    }

    #[tokio::test]
    async fn enrolled_write_and_its_outbox_row_commit_together() {
        let pool = test_pool().await;
        let hostel = seed_hostel(&pool).await;
        link(&pool, hostel.id).await;
        let before = pool.pending_sync_count(hostel.id).await.unwrap();
        seed_room(&pool, hostel.id, "7", "single").await;
        assert_eq!(
            pool.pending_sync_count(hostel.id).await.unwrap(),
            before + 1
        );

        // A failed write (duplicate room number) leaves no outbox row behind.
        let duplicate = pool
            .add_room(NewRoom {
                hostel_id: hostel.id,
                room_number: "7".into(),
                room_type: "single".into(),
                capacity: 1,
                floor: None,
                semester_rate: "100".into(),
                recess_rate: "50".into(),
            })
            .await;
        assert!(duplicate.is_err());
        assert_eq!(
            pool.pending_sync_count(hostel.id).await.unwrap(),
            before + 1
        );
    }

    #[tokio::test]
    async fn pulled_page_applies_rows_and_cursor_together() {
        let pool = test_pool().await;
        let hostel = seed_hostel(&pool).await;
        let hostel_public_id = pool.hostel_public_id(hostel.id).await.unwrap().unwrap();
        let room = |status: &str| {
            Entity::Room(RoomSnap {
                public_id: "room-from-server".into(),
                hostel_public_id: hostel_public_id.clone(),
                revision: 1,
                room_number: "9".into(),
                room_type: "single".into(),
                capacity: 1,
                floor: None,
                semester_rate: "100".into(),
                recess_rate: "50".into(),
                status: status.into(),
            })
        };
        pool.apply_remote_page(&hostel_public_id, &[room("maintenance")], 4)
            .await
            .expect("apply");
        // A later page taking the room out of maintenance is honoured.
        pool.apply_remote_page(&hostel_public_id, &[room("vacant")], 5)
            .await
            .expect("apply");
        let status: String =
            sqlx::query_scalar("SELECT status FROM rooms WHERE public_id = 'room-from-server'")
                .fetch_one(&pool.inner)
                .await
                .expect("room");
        assert_eq!(status, "vacant");
        let state = pool.sync_state(hostel.id).await.unwrap().unwrap();
        assert_eq!(state.cursor, "5");

        // A page that fails to apply moves neither rows nor the cursor.
        let bad = Entity::Room(RoomSnap {
            public_id: "orphan".into(),
            hostel_public_id: "unknown-hostel".into(),
            revision: 1,
            room_number: "10".into(),
            room_type: "single".into(),
            capacity: 1,
            floor: None,
            semester_rate: "100".into(),
            recess_rate: "50".into(),
            status: "vacant".into(),
        });
        assert!(pool
            .apply_remote_page(&hostel_public_id, &[room("maintenance"), bad], 6)
            .await
            .is_err());
        let state = pool.sync_state(hostel.id).await.unwrap().unwrap();
        assert_eq!(state.cursor, "5");
    }
}
