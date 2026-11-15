//! Local sync queue and application of rows pulled from the server.
//!
//! Integer primary keys stay local. Commands and pulled rows speak `public_id`.

use crate::DbPool;
use argon2::password_hash::rand_core::{OsRng, RngCore};
use protocol::{Entity, SyncCommand};
use sqlx::Row;
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
    /// Receipt prefix assigned at enroll, for example `AB12`.
    pub device_prefix: Option<String>,
    /// Server the desk syncs with.
    pub server_url: Option<String>,
    /// Last signed offline lease, whether or not it verified.
    pub lease_text: Option<String>,
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

/// A command the server rejected.
#[derive(Debug, Clone)]
pub struct SyncConflict {
    /// Idempotency key of the rejected command.
    pub idempotency_key: String,
    /// Server message, when one was stored.
    pub message: String,
}

impl DbPool {
    /// Append `command` to the outbox. The key is generated here.
    #[instrument(skip(self, command))]
    pub async fn enqueue_command(
        &self,
        hostel_id: i64,
        command: &SyncCommand,
    ) -> anyhow::Result<i64> {
        let payload = serde_json::to_string(command)?;
        let id = sqlx::query(
            r#"
            INSERT INTO sync_outbox (hostel_id, idempotency_key, command_type, payload)
            VALUES (?1, ?2, ?3, ?4)
            "#,
        )
        .bind(hostel_id)
        .bind(new_idempotency_key())
        .bind(command_type(command))
        .bind(payload)
        .execute(&self.inner)
        .await
        .inspect_err(|e| error!(error = %e, hostel_id, "database error enqueueing sync command"))?
        .last_insert_rowid();
        Ok(id)
    }

    /// Queue the current hostel profile. Creates send `base_revision` 0.
    pub async fn enqueue_hostel(&self, hostel_id: i64) -> anyhow::Result<()> {
        let row = sqlx::query(
            "SELECT public_id, revision, name, address, phone, currency FROM hostels WHERE id = ?1",
        )
        .bind(hostel_id)
        .fetch_optional(&self.inner)
        .await?;
        let Some(row) = row else {
            anyhow::bail!("hostel not found");
        };
        let revision: i64 = row.try_get("revision")?;
        let command = SyncCommand::UpsertHostel {
            public_id: row.try_get("public_id")?,
            base_revision: base_revision(revision),
            name: row.try_get("name")?,
            address: row.try_get("address")?,
            phone: row.try_get("phone")?,
            currency: row.try_get("currency")?,
        };
        self.enqueue_command(hostel_id, &command).await?;
        Ok(())
    }

    /// Queue the current room row.
    pub async fn enqueue_room(&self, hostel_id: i64, room_id: i64) -> anyhow::Result<()> {
        let hostel_public_id = self.require_hostel_public_id(hostel_id).await?;
        let row = sqlx::query(
            r#"
            SELECT public_id, revision, room_number, room_type, capacity, floor,
                   semester_rate, recess_rate, status
            FROM rooms WHERE id = ?1 AND hostel_id = ?2
            "#,
        )
        .bind(room_id)
        .bind(hostel_id)
        .fetch_optional(&self.inner)
        .await?;
        let Some(row) = row else {
            anyhow::bail!("room not found");
        };
        let revision: i64 = row.try_get("revision")?;
        let command = SyncCommand::UpsertRoom {
            public_id: row.try_get("public_id")?,
            hostel_public_id,
            base_revision: base_revision(revision),
            room_number: row.try_get("room_number")?,
            room_type: row.try_get("room_type")?,
            capacity: row.try_get("capacity")?,
            floor: row.try_get("floor")?,
            semester_rate: row.try_get("semester_rate")?,
            recess_rate: row.try_get("recess_rate")?,
            status: row.try_get("status")?,
        };
        self.enqueue_command(hostel_id, &command).await?;
        Ok(())
    }

    /// Queue the current resident row.
    pub async fn enqueue_resident(&self, hostel_id: i64, resident_id: i64) -> anyhow::Result<()> {
        let hostel_public_id = self.require_hostel_public_id(hostel_id).await?;
        let row = sqlx::query(
            r#"
            SELECT public_id, revision, full_name, phone, email, id_number, next_of_kin, next_of_kin_phone
            FROM residents WHERE id = ?1 AND hostel_id = ?2
            "#,
        )
        .bind(resident_id)
        .bind(hostel_id)
        .fetch_optional(&self.inner)
        .await?;
        let Some(row) = row else {
            anyhow::bail!("resident not found");
        };
        let revision: i64 = row.try_get("revision")?;
        let command = SyncCommand::UpsertResident {
            public_id: row.try_get("public_id")?,
            hostel_public_id,
            base_revision: base_revision(revision),
            full_name: row.try_get("full_name")?,
            phone: row.try_get("phone")?,
            email: row.try_get("email")?,
            id_number: row.try_get("id_number")?,
            next_of_kin: row.try_get("next_of_kin")?,
            next_of_kin_phone: row.try_get("next_of_kin_phone")?,
        };
        self.enqueue_command(hostel_id, &command).await?;
        Ok(())
    }

    /// Queue the current preferences row.
    pub async fn enqueue_preferences(&self, hostel_id: i64) -> anyhow::Result<()> {
        let hostel_public_id = self.require_hostel_public_id(hostel_id).await?;
        let row = sqlx::query(
            "SELECT revision, date_format, language, receipt_footer FROM app_preferences WHERE hostel_id = ?1",
        )
        .bind(hostel_id)
        .fetch_optional(&self.inner)
        .await?;
        let Some(row) = row else {
            anyhow::bail!("preferences not found");
        };
        let revision: i64 = row.try_get("revision")?;
        let command = SyncCommand::UpsertPreferences {
            hostel_public_id,
            base_revision: base_revision(revision),
            date_format: row.try_get("date_format")?,
            language: row.try_get("language")?,
            receipt_footer: row.try_get("receipt_footer")?,
        };
        self.enqueue_command(hostel_id, &command).await?;
        Ok(())
    }

    /// Queue a check-in that is already stored locally.
    pub async fn enqueue_check_in(&self, hostel_id: i64, allocation_id: i64) -> anyhow::Result<()> {
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
                   a.academic_year AS academic_year
            FROM allocations a
            JOIN hostels h ON h.id = a.hostel_id
            JOIN residents res ON res.id = a.resident_id
            JOIN rooms ON rooms.id = a.room_id
            WHERE a.id = ?1 AND a.hostel_id = ?2
            "#,
        )
        .bind(allocation_id)
        .bind(hostel_id)
        .fetch_optional(&self.inner)
        .await?;
        let Some(row) = row else {
            anyhow::bail!("allocation not found");
        };
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
        };
        self.enqueue_command(hostel_id, &command).await?;
        Ok(())
    }

    /// Queue a check-out that is already stored locally.
    pub async fn enqueue_check_out(
        &self,
        hostel_id: i64,
        allocation_id: i64,
    ) -> anyhow::Result<()> {
        let hostel_public_id = self.require_hostel_public_id(hostel_id).await?;
        let allocation_public_id: Option<String> = sqlx::query_scalar(
            "SELECT public_id FROM allocations WHERE id = ?1 AND hostel_id = ?2",
        )
        .bind(allocation_id)
        .bind(hostel_id)
        .fetch_optional(&self.inner)
        .await?;
        let Some(allocation_public_id) = allocation_public_id else {
            anyhow::bail!("allocation not found");
        };
        let command = SyncCommand::CheckOut {
            hostel_public_id,
            allocation_public_id,
        };
        self.enqueue_command(hostel_id, &command).await?;
        Ok(())
    }

    /// Queue a payment and its printed receipt number.
    pub async fn enqueue_payment(&self, hostel_id: i64, payment_id: i64) -> anyhow::Result<()> {
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
        .fetch_optional(&self.inner)
        .await?;
        let Some(row) = row else {
            anyhow::bail!("payment not found");
        };
        let command = SyncCommand::RecordPayment {
            payment_public_id: row.try_get("payment_public_id")?,
            receipt_public_id: row.try_get("receipt_public_id")?,
            hostel_public_id: row.try_get("hostel_public_id")?,
            allocation_public_id: row.try_get("allocation_public_id")?,
            resident_public_id: row.try_get("resident_public_id")?,
            amount: row.try_get("amount")?,
            payment_method: row.try_get("payment_method")?,
            receipt_number: row.try_get("receipt_number")?,
            notes: row.try_get("notes")?,
        };
        self.enqueue_command(hostel_id, &command).await?;
        Ok(())
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
            let command = serde_json::from_str(&payload)?;
            pending.push(PendingSync {
                id: row.try_get("id")?,
                idempotency_key: row.try_get("idempotency_key")?,
                command,
            });
        }
        Ok(pending)
    }

    /// Conflicts still waiting for someone to read them, newest last.
    pub async fn sync_conflicts(&self, hostel_id: i64) -> anyhow::Result<Vec<SyncConflict>> {
        let rows = sqlx::query(
            r#"
            SELECT idempotency_key, error
            FROM sync_outbox
            WHERE hostel_id = ?1 AND status = 'conflict'
            ORDER BY id
            "#,
        )
        .bind(hostel_id)
        .fetch_all(&self.inner)
        .await?;
        let mut conflicts = Vec::with_capacity(rows.len());
        for row in rows {
            let message: Option<String> = row.try_get("error")?;
            conflicts.push(SyncConflict {
                idempotency_key: row.try_get("idempotency_key")?,
                message: message.unwrap_or_default(),
            });
        }
        Ok(conflicts)
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

    /// Mark one outbox row `acked` or `conflict`.
    pub async fn set_outbox_status(
        &self,
        id: i64,
        status: &str,
        message: Option<&str>,
    ) -> anyhow::Result<()> {
        sqlx::query("UPDATE sync_outbox SET status = ?1, error = ?2 WHERE id = ?3")
            .bind(status)
            .bind(message)
            .bind(id)
            .execute(&self.inner)
            .await?;
        Ok(())
    }

    /// Local id of the allocation with `public_id`, when it exists.
    pub async fn allocation_id_by_public_id(
        &self,
        hostel_id: i64,
        public_id: &str,
    ) -> anyhow::Result<Option<i64>> {
        let id = sqlx::query_scalar(
            "SELECT id FROM allocations WHERE hostel_id = ?1 AND public_id = ?2",
        )
        .bind(hostel_id)
        .bind(public_id)
        .fetch_optional(&self.inner)
        .await?;
        Ok(id)
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

    /// Read sync registration for a hostel.
    pub async fn sync_state(&self, hostel_id: i64) -> anyhow::Result<Option<SyncState>> {
        let row = sqlx::query_as::<_, SyncState>(
            r#"
            SELECT hostel_id, cursor, device_id, device_prefix, server_url, lease_text, next_receipt
            FROM sync_state WHERE hostel_id = ?1
            "#,
        )
        .bind(hostel_id)
        .fetch_optional(&self.inner)
        .await?;
        Ok(row)
    }

    /// Store the device prefix and server without moving the cursor or receipt sequence.
    pub async fn save_device(
        &self,
        hostel_id: i64,
        server_url: &str,
        device_id: &str,
        device_prefix: &str,
    ) -> anyhow::Result<()> {
        sqlx::query(
            r#"
            INSERT INTO sync_state (hostel_id, server_url, device_id, device_prefix)
            VALUES (?1, ?2, ?3, ?4)
            ON CONFLICT(hostel_id) DO UPDATE SET
                server_url = excluded.server_url,
                device_id = excluded.device_id,
                device_prefix = excluded.device_prefix,
                updated_at = CURRENT_TIMESTAMP
            "#,
        )
        .bind(hostel_id)
        .bind(server_url)
        .bind(device_id)
        .bind(device_prefix)
        .execute(&self.inner)
        .await?;
        Ok(())
    }

    /// Remember the last cursor applied for a hostel.
    pub async fn save_sync_cursor(&self, hostel_id: i64, cursor: i64) -> anyhow::Result<()> {
        sqlx::query(
            r#"
            INSERT INTO sync_state (hostel_id, cursor)
            VALUES (?1, ?2)
            ON CONFLICT(hostel_id) DO UPDATE SET
                cursor = excluded.cursor,
                updated_at = CURRENT_TIMESTAMP
            "#,
        )
        .bind(hostel_id)
        .bind(cursor.to_string())
        .execute(&self.inner)
        .await?;
        Ok(())
    }

    /// Store the lease text from the latest pull.
    pub async fn save_lease(&self, hostel_id: i64, lease_text: &str) -> anyhow::Result<()> {
        sqlx::query(
            r#"
            INSERT INTO sync_state (hostel_id, lease_text)
            VALUES (?1, ?2)
            ON CONFLICT(hostel_id) DO UPDATE SET
                lease_text = excluded.lease_text,
                updated_at = CURRENT_TIMESTAMP
            "#,
        )
        .bind(hostel_id)
        .bind(lease_text)
        .execute(&self.inner)
        .await?;
        Ok(())
    }

    /// Apply pulled rows in one transaction. Existing local integer keys are reused.
    ///
    /// When the server hostel id differs from the single local hostel, that
    /// local row adopts the server id and pending commands are rewritten.
    #[instrument(skip(self, entities))]
    pub async fn apply_remote_entities(&self, entities: &[Entity]) -> anyhow::Result<()> {
        let mut ordered: Vec<&Entity> = entities.iter().collect();
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
        tx.commit().await?;
        Ok(())
    }

    async fn require_hostel_public_id(&self, hostel_id: i64) -> anyhow::Result<String> {
        self.hostel_public_id(hostel_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("hostel not found"))
    }
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

fn new_idempotency_key() -> String {
    let mut bytes = [0u8; 16];
    OsRng.fill_bytes(&mut bytes);
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
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

    let local: Vec<(i64, String)> = sqlx::query_as("SELECT id, public_id FROM hostels ORDER BY id")
        .fetch_all(&mut **tx)
        .await?;
    if local.len() == 1 {
        let (id, old_public_id) = &local[0];
        sqlx::query(
            r#"
            UPDATE hostels
            SET public_id = ?1, name = ?2, address = ?3, phone = ?4, currency = ?5, revision = ?6
            WHERE id = ?7
            "#,
        )
        .bind(&hostel.public_id)
        .bind(&hostel.name)
        .bind(&hostel.address)
        .bind(&hostel.phone)
        .bind(&hostel.currency)
        .bind(hostel.revision)
        .bind(id)
        .execute(&mut **tx)
        .await?;
        if old_public_id != &hostel.public_id {
            sqlx::query(
                "UPDATE sync_outbox SET payload = replace(payload, ?1, ?2) WHERE status = 'pending'",
            )
            .bind(old_public_id)
            .bind(&hostel.public_id)
            .execute(&mut **tx)
            .await?;
        }
        return Ok(());
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
    if let Some(id) = existing {
        sqlx::query(
            r#"
            UPDATE rooms
            SET room_number = ?1, room_type = ?2, capacity = ?3, floor = ?4,
                semester_rate = ?5, recess_rate = ?6, revision = ?7,
                status = CASE WHEN ?8 = 'maintenance' THEN 'maintenance' ELSE status END
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
    let status = if room.status == "maintenance" {
        "maintenance"
    } else {
        "vacant"
    };
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
    .bind(status)
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
                updated_at = CURRENT_TIMESTAMP
            WHERE id = ?6
            "#,
        )
        .bind(&allocation.status)
        .bind(&allocation.rate_agreed)
        .bind(&allocation.term)
        .bind(&allocation.academic_year)
        .bind(allocation.revision)
        .bind(id)
        .execute(&mut **tx)
        .await?;
        return Ok(());
    }
    sqlx::query(
        r#"
        INSERT INTO allocations (
            hostel_id, resident_id, room_id, status, rate_agreed, term, academic_year,
            public_id, revision
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
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
    let hostel_id = require_local_id(tx, "hostels", &payment.hostel_public_id).await?;
    let allocation_id = require_local_id(tx, "allocations", &payment.allocation_public_id).await?;
    let resident_id = require_local_id(tx, "residents", &payment.resident_public_id).await?;
    sqlx::query(
        r#"
        INSERT INTO payments (
            hostel_id, allocation_id, resident_id, amount, payment_method, notes, public_id
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
        "#,
    )
    .bind(hostel_id)
    .bind(allocation_id)
    .bind(resident_id)
    .bind(&payment.amount)
    .bind(&payment.payment_method)
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
    use protocol::{Entity, RoomSnap};

    #[tokio::test]
    async fn prefixed_receipt_uses_device_sequence() {
        let pool = test_pool().await;
        let hostel = seed_hostel(&pool).await;
        let room_id = seed_room(&pool, hostel.id, "1", "single").await;
        let resident_id = seed_resident(&pool, "Ada").await;
        let allocation_id = seed_active_allocation(&pool, hostel.id, resident_id, room_id).await;
        sqlx::query(
            "INSERT INTO sync_state (hostel_id, device_prefix, next_receipt) VALUES (?1, 'AB12', 7)",
        )
        .bind(hostel.id)
        .execute(&pool.inner)
        .await
        .expect("prefix");

        let (_, first) = pool
            .record_payment_and_receipt(NewPayment {
                hostel_id: hostel.id,
                allocation_id,
                resident_id,
                amount: "10".to_string(),
                payment_method: "cash".to_string(),
                period_start: None,
                period_end: None,
                notes: None,
            })
            .await
            .expect("first receipt");
        assert_eq!(first, "AB12-0007");

        let (_, second) = pool
            .record_payment_and_receipt(NewPayment {
                hostel_id: hostel.id,
                allocation_id,
                resident_id,
                amount: "5".to_string(),
                payment_method: "cash".to_string(),
                period_start: None,
                period_end: None,
                notes: None,
            })
            .await
            .expect("second receipt");
        assert_eq!(second, "AB12-0008");
    }

    #[tokio::test]
    async fn pulled_room_keeps_server_public_id() {
        let pool = test_pool().await;
        let hostel = seed_hostel(&pool).await;
        let hostel_public_id: String =
            sqlx::query_scalar("SELECT public_id FROM hostels WHERE id = ?1")
                .bind(hostel.id)
                .fetch_one(&pool.inner)
                .await
                .expect("public id");
        pool.apply_remote_entities(&[Entity::Room(RoomSnap {
            public_id: "room-from-server".into(),
            hostel_public_id,
            revision: 1,
            room_number: "9".into(),
            room_type: "single".into(),
            capacity: 1,
            floor: None,
            semester_rate: "100".into(),
            recess_rate: "50".into(),
            status: "vacant".into(),
        })])
        .await
        .expect("apply");
        let stored: String =
            sqlx::query_scalar("SELECT public_id FROM rooms WHERE room_number = '9'")
                .fetch_one(&pool.inner)
                .await
                .expect("room");
        assert_eq!(stored, "room-from-server");
    }
}
