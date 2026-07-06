use argon2::password_hash::{rand_core::OsRng, SaltString};
use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{migrate::MigrateDatabase, Sqlite, SqlitePool};
use std::str::FromStr;
use tracing::{debug, error, info, instrument};

/// Hash a plain-text password using Argon2id.
///
/// Returns a PHC-encoded string suitable for storage in `staff_users.password_hash`.
pub fn hash_password(password: &str) -> anyhow::Result<String> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    Ok(argon2
        .hash_password(password.as_bytes(), &salt)
        .map(|h| h.to_string())?)
}

/// Verify a plain-text password against a stored PHC hash.
///
/// Returns `true` when the password matches the hash.
pub fn verify_password(password: &str, password_hash: &str) -> anyhow::Result<bool> {
    let parsed_hash = PasswordHash::new(password_hash)?;
    Ok(Argon2::default()
        .verify_password(password.as_bytes(), &parsed_hash)
        .is_ok())
}

pub mod model;
pub use model::*;

#[derive(Clone, Debug)]
pub struct DbPool {
    pub inner: SqlitePool,
}

impl DbPool {
    /// Create or open a SQLite database file and run migrations.
    #[instrument]
    pub async fn open(database_url: &str) -> anyhow::Result<Self> {
        if !Sqlite::database_exists(database_url).await.unwrap_or(false) {
            info!(%database_url, "creating sqlite database");
            Sqlite::create_database(database_url).await?;
        }

        let options = SqliteConnectOptions::from_str(database_url)?
            .foreign_keys(true)
            .create_if_missing(true);

        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect_with(options)
            .await?;

        sqlx::migrate!("./migrations").run(&pool).await?;
        info!(%database_url, "migrations applied");

        Ok(Self { inner: pool })
    }

    #[instrument(skip(self))]
    pub async fn get_rooms(&self, hostel_id: i64) -> anyhow::Result<Vec<Room>> {
        debug!(hostel_id, "fetching rooms");
        let rooms = sqlx::query_as::<_, Room>(
            "SELECT id, hostel_id, room_number, room_type, capacity, floor, rate_per_period, status, created_at FROM rooms WHERE hostel_id = ? ORDER BY room_number"
        )
        .bind(hostel_id)
        .fetch_all(&self.inner)
        .await
        .inspect_err(|e| error!(error = %e, hostel_id, "database error fetching rooms"))?;

        Ok(rooms)
    }

    #[instrument(skip(self, resident))]
    pub async fn add_resident(&self, resident: NewResident) -> anyhow::Result<i64> {
        debug!("inserting resident");
        let id = sqlx::query(
            r#"
            INSERT INTO residents (full_name, phone, email, id_number, next_of_kin, photo_path)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            "#,
        )
        .bind(resident.full_name)
        .bind(resident.phone)
        .bind(resident.email)
        .bind(resident.id_number)
        .bind(resident.next_of_kin)
        .bind(resident.photo_path)
        .execute(&self.inner)
        .await
        .inspect_err(|e| error!(error = %e, "database error inserting resident"))?
        .last_insert_rowid();

        info!(resident_id = id, "resident inserted");
        Ok(id)
    }

    #[instrument(skip(self))]
    pub async fn create_allocation(&self, allocation: NewAllocation) -> anyhow::Result<i64> {
        debug!(
            resident_id = allocation.resident_id,
            room_id = allocation.room_id,
            "creating allocation"
        );
        let mut tx = self
            .inner
            .begin()
            .await
            .inspect_err(|e| error!(error = %e, "database error starting transaction"))?;

        // If a specific bed is requested, mark it occupied first.
        if let Some(bed_id) = allocation.bed_id {
            let rows = sqlx::query(
                "UPDATE beds SET status = 'occupied' WHERE id = ?1 AND status = 'vacant'",
            )
            .bind(bed_id)
            .execute(&mut *tx)
            .await
            .inspect_err(|e| error!(error = %e, bed_id, "database error updating bed status"))?
            .rows_affected();
            if rows == 0 {
                anyhow::bail!("selected bed is not available");
            }
        }

        let id = sqlx::query(
            r#"
            INSERT INTO allocations
                (hostel_id, resident_id, room_id, bed_id, check_in_date, check_out_date, expected_checkout, status, rate_agreed)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'active', ?8)
            "#,
        )
        .bind(allocation.hostel_id)
        .bind(allocation.resident_id)
        .bind(allocation.room_id)
        .bind(allocation.bed_id)
        .bind(allocation.check_in_date)
        .bind(allocation.check_out_date)
        .bind(allocation.expected_checkout)
        .bind(allocation.rate_agreed)
        .execute(&mut *tx)
        .await
        .inspect_err(|e| error!(error = %e, "database error inserting allocation"))?
        .last_insert_rowid();

        let _ = sqlx::query("UPDATE rooms SET status = 'occupied' WHERE id = ?1")
            .bind(allocation.room_id)
            .execute(&mut *tx)
            .await
            .inspect_err(|e| error!(error = %e, room_id = allocation.room_id, "database error updating room status"))?;

        tx.commit().await.inspect_err(
            |e| error!(error = %e, allocation_id = id, "database error committing allocation"),
        )?;

        info!(allocation_id = id, "allocation created");
        Ok(id)
    }

    #[instrument(skip(self))]
    pub async fn record_payment(&self, payment: NewPayment) -> anyhow::Result<i64> {
        debug!(allocation_id = payment.allocation_id, "recording payment");
        let id = sqlx::query(
            r#"
            INSERT INTO payments
                (hostel_id, allocation_id, resident_id, amount, payment_method, period_start, period_end, notes)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            "#,
        )
        .bind(payment.hostel_id)
        .bind(payment.allocation_id)
        .bind(payment.resident_id)
        .bind(payment.amount)
        .bind(payment.payment_method)
        .bind(payment.period_start)
        .bind(payment.period_end)
        .bind(payment.notes)
        .execute(&self.inner)
        .await
        .inspect_err(|e| error!(error = %e, "database error inserting payment"))?
        .last_insert_rowid();

        info!(payment_id = id, "payment recorded");
        Ok(id)
    }

    /// Record a payment and atomically create the next sequential receipt for it.
    ///
    /// Returns the new `payment_id` and the generated `receipt_number` as a string.
    /// The receipt number is allocated per hostel using `BEGIN IMMEDIATE` so that
    /// concurrent writes are serialized and the sequence stays gapless under
    /// successful commits.
    #[instrument(skip(self))]
    pub async fn record_payment_and_receipt(
        &self,
        payment: NewPayment,
    ) -> anyhow::Result<(i64, String)> {
        debug!(
            allocation_id = payment.allocation_id,
            "recording payment with receipt"
        );
        let mut conn = self
            .inner
            .acquire()
            .await
            .inspect_err(|e| error!(error = %e, "database error acquiring connection"))?;

        let run = async {
            sqlx::query("BEGIN IMMEDIATE")
                .execute(&mut *conn)
                .await
                .inspect_err(
                    |e| error!(error = %e, "database error beginning immediate transaction"),
                )?;

            let next_number: i64 = sqlx::query_scalar(
                "SELECT COALESCE(MAX(CAST(receipt_number AS INTEGER)), 0) + 1 FROM receipts WHERE hostel_id = ?1"
            )
            .bind(payment.hostel_id)
            .fetch_one(&mut *conn)
            .await
            .inspect_err(|e| error!(error = %e, hostel_id = payment.hostel_id, "database error allocating receipt number"))?;

            let receipt_number = next_number.to_string();

            let payment_id = sqlx::query(
                r#"
                INSERT INTO payments
                    (hostel_id, allocation_id, resident_id, amount, payment_method, period_start, period_end, notes)
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                "#,
            )
            .bind(payment.hostel_id)
            .bind(payment.allocation_id)
            .bind(payment.resident_id)
            .bind(payment.amount)
            .bind(&payment.payment_method)
            .bind(payment.period_start)
            .bind(payment.period_end)
            .bind(payment.notes)
            .execute(&mut *conn)
            .await
            .inspect_err(|e| error!(error = %e, "database error inserting payment in receipt transaction"))?
            .last_insert_rowid();

            sqlx::query(
                "INSERT INTO receipts (hostel_id, payment_id, receipt_number) VALUES (?1, ?2, ?3)",
            )
            .bind(payment.hostel_id)
            .bind(payment_id)
            .bind(&receipt_number)
            .execute(&mut *conn)
            .await
            .inspect_err(|e| error!(error = %e, payment_id, "database error inserting receipt"))?;

            sqlx::query("COMMIT")
                .execute(&mut *conn)
                .await
                .inspect_err(|e| error!(error = %e, payment_id, "database error committing receipt transaction"))?;

            Ok((payment_id, receipt_number))
        };

        let result: anyhow::Result<(i64, String)> = run.await;
        if result.is_err() {
            let _ = sqlx::query("ROLLBACK").execute(&mut *conn).await;
        }

        if let Ok((payment_id, ref receipt_number)) = result {
            info!(payment_id, %receipt_number, "payment recorded with receipt");
        }

        result
    }

    /// Insert a new staff user after validating the role.
    #[instrument(skip(self, user))]
    pub async fn create_staff_user(&self, user: NewStaffUser) -> anyhow::Result<i64> {
        if !matches!(user.role.as_str(), "admin" | "manager") {
            anyhow::bail!("role must be 'admin' or 'manager'");
        }

        debug!(hostel_id = user.hostel_id, username = %user.username, "inserting staff user");
        let id = sqlx::query(
            r#"
            INSERT INTO staff_users (hostel_id, name, username, password_hash, role)
            VALUES (?1, ?2, ?3, ?4, ?5)
            "#,
        )
        .bind(user.hostel_id)
        .bind(user.name)
        .bind(user.username)
        .bind(user.password_hash)
        .bind(user.role)
        .execute(&self.inner)
        .await
        .inspect_err(|e| error!(error = %e, "database error inserting staff user"))?
        .last_insert_rowid();

        info!(staff_user_id = id, "staff user inserted");
        Ok(id)
    }

    /// Authenticate a staff user by username and password.
    ///
    /// Returns the matching user (without their password hash) when credentials are valid.
    #[instrument(skip(self, password))]
    pub async fn authenticate_staff_user(
        &self,
        username: &str,
        password: &str,
    ) -> anyhow::Result<Option<StaffUser>> {
        debug!(%username, "authenticating staff user");

        let rows = sqlx::query_as::<_, StaffUser>(
            r#"
            SELECT id, hostel_id, name, username, role, created_at
            FROM staff_users
            WHERE username = ?1
            "#,
        )
        .bind(username)
        .fetch_all(&self.inner)
        .await
        .inspect_err(|e| error!(error = %e, %username, "database error fetching staff user"))?;

        for user in rows {
            // Retrieve the stored hash for this candidate user.
            let hash: Option<String> = sqlx::query_scalar(
                "SELECT password_hash FROM staff_users WHERE id = ?1",
            )
            .bind(user.id)
            .fetch_optional(&self.inner)
            .await
            .inspect_err(
                |e| error!(error = %e, user_id = user.id, "database error fetching password hash"),
            )?;

            if let Some(hash) = hash {
                if verify_password(password, &hash)? {
                    info!(user_id = user.id, "staff user authenticated");
                    return Ok(Some(user));
                }
            }
        }

        debug!(%username, "authentication failed");
        Ok(None)
    }

    /// Insert a new hostel and return the full row (id, created_at, currency).
    ///
    /// `currency` defaults to `"USD"` when `None`. Other fields are optional.
    #[instrument(skip(self, hostel))]
    pub async fn create_hostel(&self, hostel: NewHostel) -> anyhow::Result<Hostel> {
        debug!(name = %hostel.name, "creating hostel");
        let row = sqlx::query_as::<_, Hostel>(
            r#"
            INSERT INTO hostels (name, address, phone, currency, branding_logo_path)
            VALUES (?1, ?2, ?3, ?4, ?5)
            RETURNING id, name, address, phone, currency, branding_logo_path, created_at
            "#,
        )
        .bind(hostel.name)
        .bind(hostel.address)
        .bind(hostel.phone)
        .bind(hostel.currency.as_deref().unwrap_or("USD"))
        .bind(hostel.branding_logo_path)
        .fetch_one(&self.inner)
        .await
        .inspect_err(|e| error!(error = %e, "database error inserting hostel"))?;

        info!(hostel_id = row.id, "hostel inserted");
        Ok(row)
    }

    /// Insert a single room with `status = 'vacant'` and return its id.
    ///
    /// Validates `capacity > 0` and `room_type` ∈ {single, double, dorm}.
    #[instrument(skip(self, room))]
    pub async fn add_room(&self, room: NewRoom) -> anyhow::Result<i64> {
        debug!(hostel_id = room.hostel_id, room_number = %room.room_number, "adding room");
        let mut tx = self
            .inner
            .begin()
            .await
            .inspect_err(|e| error!(error = %e, "database error starting transaction"))?;
        let id = insert_room(&mut tx, &room).await?;
        tx.commit()
            .await
            .inspect_err(|e| error!(error = %e, "database error committing room"))?;
        info!(room_id = id, "room inserted");
        Ok(id)
    }

    /// Atomically insert many rooms in one transaction.
    ///
    /// If any insert fails the whole batch is rolled back. Returns the ids of
    /// the inserted rooms in input order.
    #[instrument(skip(self, rooms))]
    pub async fn bulk_add_rooms(&self, rooms: Vec<NewRoom>) -> anyhow::Result<Vec<i64>> {
        debug!(count = rooms.len(), "bulk adding rooms");
        let mut tx = self
            .inner
            .begin()
            .await
            .inspect_err(|e| error!(error = %e, "database error starting transaction"))?;
        let mut ids = Vec::with_capacity(rooms.len());
        for room in &rooms {
            ids.push(insert_room(&mut tx, room).await?);
        }
        tx.commit()
            .await
            .inspect_err(|e| error!(error = %e, "database error committing bulk rooms"))?;
        info!(count = ids.len(), "bulk rooms inserted");
        Ok(ids)
    }

    /// Return the hostel profile with derived `room_count`, `admin_count`,
    /// and `onboarding_complete` flag, or `None` if the hostel doesn't exist.
    #[instrument(skip(self))]
    pub async fn get_hostel_profile(
        &self,
        hostel_id: i64,
    ) -> anyhow::Result<Option<HostelProfile>> {
        debug!(hostel_id, "fetching hostel profile");
        let row = sqlx::query_as::<_, HostelProfile>(
            r#"
            SELECT
              h.id AS id, h.name AS name, h.address AS address, h.phone AS phone,
              h.currency AS currency, h.branding_logo_path AS branding_logo_path,
              (SELECT COUNT(*) FROM rooms r WHERE r.hostel_id = h.id) AS room_count,
              (SELECT COUNT(*) FROM staff_users s WHERE s.hostel_id = h.id AND s.role = 'admin') AS admin_count,
              0 AS onboarding_complete
            FROM hostels h
            WHERE h.id = ?1
            "#,
        )
        .bind(hostel_id)
        .fetch_optional(&self.inner)
        .await
        .inspect_err(|e| error!(error = %e, hostel_id, "database error fetching hostel profile"))?;

        match row {
            Some(mut p) => {
                p.onboarding_complete = p.room_count > 0 && p.admin_count > 0;
                Ok(Some(p))
            }
            None => Ok(None),
        }
    }

    /// Return all hostels ordered by id.
    #[instrument(skip(self))]
    pub async fn list_hostels(&self) -> anyhow::Result<Vec<Hostel>> {
        debug!("listing hostels");
        let rows = sqlx::query_as::<_, Hostel>(
            "SELECT id, name, address, phone, currency, branding_logo_path, created_at FROM hostels ORDER BY id",
        )
        .fetch_all(&self.inner)
        .await
        .inspect_err(|e| error!(error = %e, "database error listing hostels"))?;
        Ok(rows)
    }
}

/// Insert a room on the given transaction, validating capacity and room_type.
///
/// Shared by [`DbPool::add_room`] and [`DbPool::bulk_add_rooms`] so both paths
/// apply the same validation and SQL.
async fn insert_room(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    room: &NewRoom,
) -> anyhow::Result<i64> {
    if room.capacity <= 0 {
        anyhow::bail!("capacity must be greater than zero");
    }
    if !matches!(room.room_type.as_str(), "single" | "double" | "dorm") {
        anyhow::bail!("room_type must be one of: single, double, dorm");
    }
    let id = sqlx::query(
        r#"
        INSERT INTO rooms (hostel_id, room_number, room_type, capacity, floor, rate_per_period, status)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'vacant')
        "#,
    )
    .bind(room.hostel_id)
    .bind(&room.room_number)
    .bind(&room.room_type)
    .bind(room.capacity)
    .bind(room.floor)
    .bind(&room.rate_per_period)
    .execute(&mut **tx)
    .await
    .inspect_err(|e| error!(error = %e, hostel_id = room.hostel_id, room_number = %room.room_number, "database error inserting room"))?
    .last_insert_rowid();
    Ok(id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn password_hash_and_verify_round_trip() {
        let password = "a_secure_password_123";
        let hash = hash_password(password).expect("hashing should succeed");
        assert!(
            verify_password(password, &hash).expect("verification should not fail"),
            "correct password should verify"
        );
        assert!(
            !verify_password("wrong_password", &hash).expect("verification should not fail"),
            "wrong password should not verify"
        );
    }

    /// Open an in-memory SQLite pool with migrations applied for tests.
    async fn test_pool() -> DbPool {
        DbPool::open("sqlite::memory:").await.expect("open test db")
    }

    #[tokio::test]
    async fn create_hostel_inserts_and_returns_row() {
        let pool = test_pool().await;
        let hostel = pool
            .create_hostel(NewHostel {
                name: "Sunrise Lodge".to_string(),
                address: Some("12 Main St".to_string()),
                phone: None,
                currency: None,
                branding_logo_path: None,
            })
            .await
            .expect("create hostel");
        assert!(hostel.id > 0);
        assert_eq!(hostel.name, "Sunrise Lodge");
        assert_eq!(hostel.currency, "USD");
        assert_eq!(hostel.address.as_deref(), Some("12 Main St"));
    }

    #[tokio::test]
    async fn add_room_creates_vacant_room() {
        let pool = test_pool().await;
        let hostel = pool
            .create_hostel(NewHostel {
                name: "H".to_string(),
                address: None,
                phone: None,
                currency: None,
                branding_logo_path: None,
            })
            .await
            .expect("create hostel");
        let id = pool
            .add_room(NewRoom {
                hostel_id: hostel.id,
                room_number: "101".to_string(),
                room_type: "single".to_string(),
                capacity: 2,
                floor: Some(1),
                rate_per_period: "50.00".to_string(),
            })
            .await
            .expect("add room");
        assert!(id > 0);
        let rooms = pool.get_rooms(hostel.id).await.expect("fetch rooms");
        assert_eq!(rooms.len(), 1);
        assert_eq!(rooms[0].status, "vacant");
        assert_eq!(rooms[0].room_number, "101");
        assert_eq!(rooms[0].rate_per_period, "50.00");
    }

    #[tokio::test]
    async fn add_room_rejects_duplicate_room_number() {
        let pool = test_pool().await;
        let hostel = pool
            .create_hostel(NewHostel {
                name: "H".to_string(),
                address: None,
                phone: None,
                currency: None,
                branding_logo_path: None,
            })
            .await
            .expect("create hostel");
        let room = NewRoom {
            hostel_id: hostel.id,
            room_number: "101".to_string(),
            room_type: "single".to_string(),
            capacity: 1,
            floor: None,
            rate_per_period: "10".to_string(),
        };
        pool.add_room(room.clone()).await.expect("first insert");
        let err = pool
            .add_room(room)
            .await
            .expect_err("duplicate should fail");
        assert!(
            err.to_string().contains("UNIQUE") || err.to_string().contains("constraint"),
            "expected constraint violation, got: {err}"
        );
    }

    #[tokio::test]
    async fn add_room_rejects_zero_capacity() {
        let pool = test_pool().await;
        let hostel = pool
            .create_hostel(NewHostel {
                name: "H".to_string(),
                address: None,
                phone: None,
                currency: None,
                branding_logo_path: None,
            })
            .await
            .expect("create hostel");
        let err = pool
            .add_room(NewRoom {
                hostel_id: hostel.id,
                room_number: "101".to_string(),
                room_type: "single".to_string(),
                capacity: 0,
                floor: None,
                rate_per_period: "10".to_string(),
            })
            .await
            .expect_err("zero capacity should fail");
        assert!(err.to_string().contains("capacity"), "got: {err}");
    }

    #[tokio::test]
    async fn bulk_add_rooms_atomic() {
        let pool = test_pool().await;
        let hostel = pool
            .create_hostel(NewHostel {
                name: "H".to_string(),
                address: None,
                phone: None,
                currency: None,
                branding_logo_path: None,
            })
            .await
            .expect("create hostel");
        // Seed an existing room so the third item in the batch is a duplicate.
        pool.add_room(NewRoom {
            hostel_id: hostel.id,
            room_number: "101".to_string(),
            room_type: "single".to_string(),
            capacity: 1,
            floor: None,
            rate_per_period: "10".to_string(),
        })
        .await
        .expect("seed room");

        let rooms = vec![
            NewRoom {
                hostel_id: hostel.id,
                room_number: "102".to_string(),
                room_type: "single".to_string(),
                capacity: 1,
                floor: None,
                rate_per_period: "10".to_string(),
            },
            NewRoom {
                hostel_id: hostel.id,
                room_number: "103".to_string(),
                room_type: "double".to_string(),
                capacity: 2,
                floor: None,
                rate_per_period: "20".to_string(),
            },
            NewRoom {
                hostel_id: hostel.id,
                room_number: "101".to_string(), // duplicate
                room_type: "dorm".to_string(),
                capacity: 4,
                floor: None,
                rate_per_period: "5".to_string(),
            },
        ];
        let err = pool
            .bulk_add_rooms(rooms)
            .await
            .expect_err("batch should fail");
        assert!(
            err.to_string().contains("UNIQUE") || err.to_string().contains("constraint"),
            "got: {err}"
        );
        // No new rooms should have been committed.
        let after = pool.get_rooms(hostel.id).await.expect("fetch rooms");
        assert_eq!(after.len(), 1, "rollback should leave only the seeded room");
    }

    #[tokio::test]
    async fn get_hostel_profile_onboarding_flags() {
        let pool = test_pool().await;
        let hostel = pool
            .create_hostel(NewHostel {
                name: "H".to_string(),
                address: None,
                phone: None,
                currency: None,
                branding_logo_path: None,
            })
            .await
            .expect("create hostel");

        // Fresh hostel: no rooms, no admins.
        let profile = pool
            .get_hostel_profile(hostel.id)
            .await
            .expect("fetch profile")
            .expect("hostel exists");
        assert_eq!(profile.room_count, 0);
        assert_eq!(profile.admin_count, 0);
        assert!(!profile.onboarding_complete);

        // Add a room and an admin staff user.
        pool.add_room(NewRoom {
            hostel_id: hostel.id,
            room_number: "101".to_string(),
            room_type: "single".to_string(),
            capacity: 1,
            floor: None,
            rate_per_period: "10".to_string(),
        })
        .await
        .expect("add room");
        pool.create_staff_user(NewStaffUser {
            hostel_id: hostel.id,
            name: "Admin".to_string(),
            username: "admin".to_string(),
            password_hash: hash_password("pw").expect("hash"),
            role: "admin".to_string(),
        })
        .await
        .expect("add admin");

        let profile = pool
            .get_hostel_profile(hostel.id)
            .await
            .expect("fetch profile")
            .expect("hostel exists");
        assert_eq!(profile.room_count, 1);
        assert_eq!(profile.admin_count, 1);
        assert!(profile.onboarding_complete);
    }
}
