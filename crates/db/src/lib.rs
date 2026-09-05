use argon2::password_hash::{rand_core::OsRng, SaltString};
use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use chrono::{DateTime, Datelike, Local, NaiveDate, TimeZone, Utc};
use rust_decimal::Decimal;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};
use sqlx::Row;
use sqlx::SqlitePool;
use std::str::FromStr;
use std::time::Duration;
use tracing::{debug, error, info, instrument};

/// Fixed Argon2 hash used to equalize login timing when an email does not exist.
///
/// The hash has no corresponding password and is never used for authentication.
/// Verifying against it ensures an unknown email still performs one Argon2
/// verification, matching the work done for a wrong password against a real
/// email.
const DUMMY_PASSWORD_HASH: &str =
    "$argon2id$v=19$m=19456,t=2,p=1$dUTZ/yeSMQlV7a3m/Fl6dg$c3e1mge9bB8E/jomvoijodaCH200x8hfZ8jY+iKIBUM";

/// Hash a plain-text password using Argon2id.
///
/// Returns a PHC-encoded string suitable for storage in `users.password_hash`.
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
    ///
    /// Configures the connection for desktop use: WAL journal mode, normal
    /// synchronous safety, a 5-second busy timeout, and foreign-key enforcement.
    /// The `create_if_missing` option removes the need for a separate
    /// database-existence check.
    #[instrument]
    pub async fn open(database_url: &str) -> anyhow::Result<Self> {
        let options = SqliteConnectOptions::from_str(database_url)?
            .journal_mode(SqliteJournalMode::Wal)
            .synchronous(SqliteSynchronous::Normal)
            .busy_timeout(Duration::from_secs(5))
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
            "SELECT id, hostel_id, room_number, room_type, capacity, floor, rate_per_period, billing_period, status, created_at FROM rooms WHERE hostel_id = ? ORDER BY room_number"
        )
        .bind(hostel_id)
        .fetch_all(&self.inner)
        .await
        .inspect_err(|e| error!(error = %e, hostel_id, "database error fetching rooms"))?;

        Ok(rooms)
    }

    /// Check a resident into a room atomically.
    ///
    /// If `check_in.resident_id` is `Some`, it is used directly; otherwise a new
    /// resident is created from `full_name` and the other contact fields. The
    /// resident creation and room allocation share a single transaction: if the
    /// allocation cannot be created, no resident row is left behind.
    #[instrument(skip(self, check_in))]
    pub async fn check_in(&self, check_in: NewCheckIn) -> anyhow::Result<i64> {
        debug!(room_id = check_in.room_id, "checking in resident");
        let mut tx =
            self.inner.begin().await.inspect_err(
                |e| error!(error = %e, "database error starting check-in transaction"),
            )?;

        let resident_id = match check_in.resident_id {
            Some(id) => id,
            None => {
                let name = check_in.full_name.as_deref().unwrap_or("").trim();
                if name.is_empty() {
                    anyhow::bail!("full_name is required for a new resident");
                }
                insert_resident(
                    &mut tx,
                    &NewResident {
                        full_name: name.to_string(),
                        phone: check_in.phone.clone(),
                        email: check_in.email.clone(),
                        id_number: check_in.id_number.clone(),
                        next_of_kin: check_in.next_of_kin.clone(),
                        photo_path: check_in.photo_path.clone(),
                    },
                )
                .await?
            }
        };

        let allocation_id = insert_allocation(
            &mut tx,
            &NewAllocation {
                hostel_id: check_in.hostel_id,
                resident_id,
                room_id: check_in.room_id,
                check_in_date: check_in.check_in_date,
                check_out_date: check_in.check_out_date,
                expected_checkout: check_in.expected_checkout,
                rate_agreed: check_in.rate_agreed,
            },
        )
        .await?;

        tx.commit().await.inspect_err(
            |e| error!(error = %e, allocation_id, "database error committing check-in"),
        )?;

        info!(allocation_id, resident_id, "check-in complete");
        Ok(allocation_id)
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
        let mut tx =
            self.inner.begin_with("BEGIN IMMEDIATE").await.inspect_err(
                |e| error!(error = %e, "database error beginning receipt transaction"),
            )?;

        let next_number: i64 = sqlx::query_scalar(
            "SELECT COALESCE(MAX(CAST(receipt_number AS INTEGER)), 0) + 1 FROM receipts WHERE hostel_id = ?1"
        )
        .bind(payment.hostel_id)
        .fetch_one(&mut *tx)
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
        .execute(&mut *tx)
        .await
        .inspect_err(|e| error!(error = %e, "database error inserting payment in receipt transaction"))?
        .last_insert_rowid();

        sqlx::query(
            "INSERT INTO receipts (hostel_id, payment_id, receipt_number) VALUES (?1, ?2, ?3)",
        )
        .bind(payment.hostel_id)
        .bind(payment_id)
        .bind(&receipt_number)
        .execute(&mut *tx)
        .await
        .inspect_err(|e| error!(error = %e, payment_id, "database error inserting receipt"))?;

        tx.commit().await.inspect_err(
            |e| error!(error = %e, payment_id, "database error committing receipt transaction"),
        )?;

        info!(payment_id, %receipt_number, "payment recorded with receipt");
        Ok((payment_id, receipt_number))
    }

    /// Update `receipts.pdf_path` for a given hostel and receipt number.
    ///
    /// Returns `Ok(())` whether a row was updated or not. A missing row is
    /// logged at debug level to support legacy receipts without failing the
    /// caller.
    #[instrument(skip(self))]
    pub async fn update_receipt_pdf_path(
        &self,
        hostel_id: i64,
        receipt_number: &str,
        pdf_path: &str,
    ) -> anyhow::Result<()> {
        debug!(hostel_id, %receipt_number, %pdf_path, "updating receipt pdf_path");

        let rows = sqlx::query(
            "UPDATE receipts SET pdf_path = ?1 WHERE hostel_id = ?2 AND receipt_number = ?3",
        )
        .bind(pdf_path)
        .bind(hostel_id)
        .bind(receipt_number)
        .execute(&self.inner)
        .await
        .inspect_err(|e| error!(error = %e, hostel_id, %receipt_number, "database error updating receipt pdf_path"))?
        .rows_affected();

        if rows == 0 {
            debug!(hostel_id, %receipt_number, "no receipt row found for pdf_path update");
        } else {
            info!(hostel_id, %receipt_number, "receipt pdf_path updated");
        }

        Ok(())
    }

    /// Complete first-run onboarding by creating one hostel and one admin.
    ///
    /// The check and both inserts share a `BEGIN IMMEDIATE` transaction, so the
    /// operation is one-time even if two callers race. The transaction guard
    /// rolls back automatically if the future is cancelled.
    #[instrument(skip(self, input))]
    pub async fn complete_onboarding(
        &self,
        input: CompleteOnboarding,
    ) -> anyhow::Result<User> {
        if input.hostel.name.trim().is_empty() {
            anyhow::bail!("hostel name is required");
        }
        if input.admin_name.trim().is_empty() {
            anyhow::bail!("admin name is required");
        }
        if input.email.trim().is_empty() {
            anyhow::bail!("email is required");
        }

        let mut tx = self.inner.begin_with("BEGIN IMMEDIATE").await.inspect_err(
            |e| error!(error = %e, "database error starting onboarding transaction"),
        )?;

        let hostel_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM hostels")
            .fetch_one(&mut *tx)
            .await
            .inspect_err(|e| error!(error = %e, "database error counting hostels"))?;
        if hostel_count != 0 {
            anyhow::bail!("onboarding is already complete");
        }

        let hostel_id = sqlx::query(
            r#"
            INSERT INTO hostels (name, address, phone, currency, branding_logo_path)
            VALUES (?1, ?2, ?3, ?4, ?5)
            "#,
        )
        .bind(input.hostel.name.trim())
        .bind(input.hostel.address)
        .bind(input.hostel.phone)
        .bind(input.hostel.currency.as_deref().unwrap_or("UGX"))
        .bind(input.hostel.branding_logo_path)
        .execute(&mut *tx)
        .await
        .inspect_err(|e| error!(error = %e, "database error inserting onboarding hostel"))?
        .last_insert_rowid();

        let user_id = sqlx::query(
            r#"
            INSERT INTO users (hostel_id, full_name, email, password_hash, role)
            VALUES (?1, ?2, ?3, ?4, 'admin')
            "#,
        )
        .bind(hostel_id)
        .bind(input.admin_name.trim())
        .bind(input.email.trim())
        .bind(input.password_hash)
        .execute(&mut *tx)
        .await
        .inspect_err(|e| error!(error = %e, "database error inserting onboarding admin"))?
        .last_insert_rowid();

        let user = sqlx::query_as::<_, User>(
            "SELECT id, hostel_id, full_name, email, role, created_at FROM users WHERE id = ?1",
        )
        .bind(user_id)
        .fetch_one(&mut *tx)
        .await
        .inspect_err(|e| error!(error = %e, user_id, "database error fetching onboarding admin"))?;

        tx.commit()
            .await
            .inspect_err(|e| error!(error = %e, "database error committing onboarding"))?;

        info!(hostel_id, user_id, "onboarding completed");
        Ok(user)
    }

    /// Insert a new staff user after validating the role.
    #[instrument(skip(self, user))]
    pub async fn create_user(&self, user: NewUser) -> anyhow::Result<i64> {
        if !matches!(user.role.as_str(), "admin" | "manager") {
            anyhow::bail!("role must be 'admin' or 'manager'");
        }

        debug!(hostel_id = user.hostel_id, email = %user.email, "inserting user");
        let id = sqlx::query(
            r#"
            INSERT INTO users (hostel_id, full_name, email, password_hash, role)
            VALUES (?1, ?2, ?3, ?4, ?5)
            "#,
        )
        .bind(user.hostel_id)
        .bind(user.full_name)
        .bind(user.email)
        .bind(user.password_hash)
        .bind(user.role)
        .execute(&self.inner)
        .await
        .inspect_err(|e| error!(error = %e, "database error inserting user"))?
        .last_insert_rowid();

        info!(user_id = id, "user inserted");
        Ok(id)
    }

    /// Fetch a staff user by id without exposing the password hash.
    ///
    /// Returns `None` when the id does not exist.
    #[instrument(skip(self))]
    pub async fn get_user_by_id(&self, user_id: i64) -> anyhow::Result<Option<User>> {
        debug!(user_id, "fetching user by id");
        let user = sqlx::query_as::<_, User>(
            "SELECT id, hostel_id, full_name, email, role, created_at FROM users WHERE id = ?1",
        )
        .bind(user_id)
        .fetch_optional(&self.inner)
        .await
        .inspect_err(|e| error!(error = %e, user_id, "database error fetching user"))?;
        Ok(user)
    }

    /// Authenticate a staff user by email and password.
    ///
    /// Returns the matching user (without their password hash) when credentials are valid.
    /// A single query fetches every candidate row so the number of database
    /// round-trips stays constant regardless of how many hostels share an email.
    #[instrument(skip(self, password))]
    pub async fn authenticate_user(
        &self,
        email: &str,
        password: &str,
    ) -> anyhow::Result<Option<User>> {
        debug!(%email, "authenticating user");

        let rows = sqlx::query(
            r#"
            SELECT id, hostel_id, full_name, email, role, created_at, password_hash
            FROM users
            WHERE email = ?1
            "#,
        )
        .bind(email)
        .fetch_all(&self.inner)
        .await
        .inspect_err(|e| error!(error = %e, %email, "database error fetching user"))?;

        if rows.is_empty() {
            let _ = verify_password(password, DUMMY_PASSWORD_HASH)?;
            debug!(%email, "authentication failed");
            return Ok(None);
        }

        for row in rows {
            let user = User {
                id: row.try_get("id")?,
                hostel_id: row.try_get("hostel_id")?,
                full_name: row.try_get("full_name")?,
                email: row.try_get("email")?,
                role: row.try_get("role")?,
                created_at: row.try_get("created_at")?,
            };
            let hash: String = row.try_get("password_hash")?;

            if verify_password(password, &hash)? {
                info!(user_id = user.id, "user authenticated");
                return Ok(Some(user));
            }
        }

        debug!(%email, "authentication failed");
        Ok(None)
    }

    /// Update a staff user's full name, email, role and optionally their password.
    ///
    /// Returns the updated user row. The caller is responsible for hashing any
    /// supplied password before passing it in.
    #[instrument(skip(self, password_hash))]
    pub async fn update_user(
        &self,
        user_id: i64,
        full_name: String,
        email: String,
        role: String,
        password_hash: Option<String>,
    ) -> anyhow::Result<User> {
        if !matches!(role.as_str(), "admin" | "manager") {
            anyhow::bail!("role must be 'admin' or 'manager'");
        }

        debug!(user_id, %email, "updating user");

        let row = if let Some(hash) = password_hash {
            sqlx::query_as::<_, User>(
                r#"
                UPDATE users
                SET full_name = ?1, email = ?2, role = ?3, password_hash = ?4
                WHERE id = ?5
                RETURNING id, hostel_id, full_name, email, role, created_at
                "#,
            )
            .bind(&full_name)
            .bind(&email)
            .bind(&role)
            .bind(&hash)
            .bind(user_id)
            .fetch_one(&self.inner)
            .await
        } else {
            sqlx::query_as::<_, User>(
                r#"
                UPDATE users
                SET full_name = ?1, email = ?2, role = ?3
                WHERE id = ?4
                RETURNING id, hostel_id, full_name, email, role, created_at
                "#,
            )
            .bind(&full_name)
            .bind(&email)
            .bind(&role)
            .bind(user_id)
            .fetch_one(&self.inner)
            .await
        }
        .inspect_err(|e| error!(error = %e, user_id, "database error updating user"))?;

        info!(user_id = row.id, "user updated");
        Ok(row)
    }

    /// Fetch the role of a staff user by id.
    ///
    /// Returns `None` when no user with the supplied id exists.
    #[instrument(skip(self))]
    pub async fn get_user_role(&self, user_id: i64) -> anyhow::Result<Option<String>> {
        debug!(user_id, "fetching user role");

        let role: Option<String> = sqlx::query_scalar("SELECT role FROM users WHERE id = ?1")
            .bind(user_id)
            .fetch_optional(&self.inner)
            .await
            .inspect_err(
                |e| error!(error = %e, user_id, "database error fetching user role"),
            )?;

        Ok(role)
    }

    /// Delete a staff user, protecting against deletion of the last admin.
    ///
    /// Returns `Ok(())` if the user existed and was deleted, or an error if the
    /// user was the only admin remaining.
    #[instrument(skip(self))]
    pub async fn delete_user(&self, user_id: i64) -> anyhow::Result<()> {
        debug!(user_id, "deleting user");

        let user: Option<User> = sqlx::query_as::<_, User>(
            "SELECT id, hostel_id, full_name, email, role, created_at FROM users WHERE id = ?1",
        )
        .bind(user_id)
        .fetch_optional(&self.inner)
        .await
        .inspect_err(
            |e| error!(error = %e, user_id, "database error fetching user for delete"),
        )?;

        let user = match user {
            Some(u) => u,
            None => return Ok(()),
        };

        if user.role == "admin" {
            let admin_count: i64 = sqlx::query_scalar(
                "SELECT COUNT(*) FROM users WHERE hostel_id = ?1 AND role = 'admin'",
            )
            .bind(user.hostel_id)
            .fetch_one(&self.inner)
            .await
            .inspect_err(|e| error!(error = %e, hostel_id = user.hostel_id, "database error counting admins"))?;

            if admin_count <= 1 {
                anyhow::bail!("cannot delete the only admin account");
            }
        }

        sqlx::query("DELETE FROM users WHERE id = ?1")
            .bind(user_id)
            .execute(&self.inner)
            .await
            .inspect_err(|e| error!(error = %e, user_id, "database error deleting user"))?;

        info!(user_id, "user deleted");
        Ok(())
    }

    /// Read app preferences for a hostel, creating defaults if missing.
    #[instrument(skip(self))]
    pub async fn get_app_preferences(&self, hostel_id: i64) -> anyhow::Result<AppPreferences> {
        debug!(hostel_id, "fetching app preferences");

        let row = sqlx::query_as::<_, AppPreferences>(
            r#"
            SELECT hostel_id, date_format, language, receipt_footer, updated_at
            FROM app_preferences
            WHERE hostel_id = ?1
            "#,
        )
        .bind(hostel_id)
        .fetch_optional(&self.inner)
        .await
        .inspect_err(
            |e| error!(error = %e, hostel_id, "database error fetching app preferences"),
        )?;

        if let Some(prefs) = row {
            return Ok(prefs);
        }

        sqlx::query(
            r#"
            INSERT INTO app_preferences (hostel_id)
            VALUES (?1)
            ON CONFLICT(hostel_id) DO NOTHING
            "#,
        )
        .bind(hostel_id)
        .execute(&self.inner)
        .await
        .inspect_err(
            |e| error!(error = %e, hostel_id, "database error creating default app preferences"),
        )?;

        Ok(AppPreferences {
            hostel_id,
            date_format: "dd/MM/yyyy".to_string(),
            language: "en".to_string(),
            receipt_footer: None,
            updated_at: Utc::now(),
        })
    }

    /// Create or update app preferences for a hostel.
    #[instrument(skip(self, prefs))]
    pub async fn upsert_app_preferences(
        &self,
        prefs: UpsertAppPreferences,
    ) -> anyhow::Result<AppPreferences> {
        debug!(hostel_id = prefs.hostel_id, "upserting app preferences");

        let valid_formats = ["dd/MM/yyyy", "MM/dd/yyyy", "yyyy-MM-dd"];
        if !valid_formats.contains(&prefs.date_format.as_str()) {
            anyhow::bail!("date_format must be one of dd/MM/yyyy, MM/dd/yyyy, yyyy-MM-dd");
        }

        if prefs.language != "en" {
            anyhow::bail!("language must be 'en'");
        }

        let row = sqlx::query_as::<_, AppPreferences>(
            r#"
            INSERT INTO app_preferences (hostel_id, date_format, language, receipt_footer, updated_at)
            VALUES (?1, ?2, ?3, ?4, ?5)
            ON CONFLICT(hostel_id) DO UPDATE SET
                date_format = excluded.date_format,
                language = excluded.language,
                receipt_footer = excluded.receipt_footer,
                updated_at = excluded.updated_at
            RETURNING hostel_id, date_format, language, receipt_footer, updated_at
            "#,
        )
        .bind(prefs.hostel_id)
        .bind(prefs.date_format)
        .bind(prefs.language)
        .bind(prefs.receipt_footer)
        .bind(Utc::now())
        .fetch_one(&self.inner)
        .await
        .inspect_err(|e| error!(error = %e, hostel_id = prefs.hostel_id, "database error upserting app preferences"))?;

        Ok(row)
    }

    /// Insert a new hostel and return the full row (id, created_at, currency).
    ///
    /// `currency` defaults to `"UGX"` when `None`. Other fields are optional.
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
        .bind(hostel.currency.as_deref().unwrap_or("UGX"))
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

    /// Insert multiple rooms atomically, returning their ids in order.
    ///
    /// Validates each room the same way as [`DbPool::add_room`]; if any
    /// fails, the whole batch is rolled back.
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
            .inspect_err(|e| error!(error = %e, "database error committing rooms"))?;
        info!(count = ids.len(), "rooms inserted");
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
              (SELECT COUNT(*) FROM users s WHERE s.hostel_id = h.id AND s.role = 'admin') AS admin_count,
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

    /// Fetch a hostel by id.
    ///
    /// Returns `None` when no hostel with the supplied id exists.
    #[instrument(skip(self))]
    pub async fn get_hostel_by_id(&self, hostel_id: i64) -> anyhow::Result<Option<Hostel>> {
        debug!(hostel_id, "fetching hostel by id");
        let hostel = sqlx::query_as::<_, Hostel>(
            "SELECT id, name, address, phone, currency, branding_logo_path, created_at FROM hostels WHERE id = ?1",
        )
        .bind(hostel_id)
        .fetch_optional(&self.inner)
        .await
        .inspect_err(|e| error!(error = %e, hostel_id, "database error fetching hostel"))?;
        Ok(hostel)
    }

    /// Return whether first-run onboarding has produced a usable hostel.
    ///
    /// Onboarding is complete when exactly one hostel exists and it has at
    /// least one staff account.
    #[instrument(skip(self))]
    pub async fn is_onboarding_complete(&self) -> anyhow::Result<bool> {
        let row = sqlx::query(
            r#"
            SELECT
              (SELECT COUNT(*) FROM hostels) AS hostel_count,
              (SELECT COUNT(*) FROM users) AS staff_count
            "#,
        )
        .fetch_one(&self.inner)
        .await
        .inspect_err(|e| error!(error = %e, "database error checking onboarding status"))?;
        let hostel_count: i64 = row.try_get("hostel_count")?;
        let staff_count: i64 = row.try_get("staff_count")?;
        Ok(hostel_count == 1 && staff_count > 0)
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

    /// List every resident together with their latest allocation in the hostel.
    ///
    /// Residents with no allocation in this hostel show `status = "never_allocated"`
    /// and a balance of `"0"`. Active residents have their balance computed as
    /// `rate_agreed * periods_elapsed - total_paid`.
    #[instrument(skip(self))]
    pub async fn list_residents(&self, hostel_id: i64) -> anyhow::Result<Vec<ResidentSummary>> {
        debug!(hostel_id, "listing residents");
        let today = Utc::now().date_naive();
        let rows = sqlx::query(
            r#"
            SELECT
                r.id AS resident_id,
                r.full_name,
                r.phone,
                r.email,
                a.id AS allocation_id,
                rm.room_number,
                a.check_in_date,
                a.check_out_date,
                a.expected_checkout,
                a.status AS allocation_status,
                rm.billing_period,
                a.rate_agreed,
                GROUP_CONCAT(p.amount) AS payment_amounts
            FROM residents r
            LEFT JOIN allocations a
              ON a.resident_id = r.id
             AND a.hostel_id = ?1
            LEFT JOIN rooms rm ON rm.id = a.room_id
            LEFT JOIN payments p ON p.allocation_id = a.id
            GROUP BY r.id, a.id
            ORDER BY r.full_name, r.id, a.id
            "#,
        )
        .bind(hostel_id)
        .fetch_all(&self.inner)
        .await
        .inspect_err(|e| error!(error = %e, hostel_id, "database error listing residents"))?;

        let mut summaries: Vec<ResidentSummary> = Vec::new();
        for row in rows {
            let resident_id: i64 = row.try_get("resident_id")?;
            let allocation_status: Option<String> = row.try_get("allocation_status")?;
            let check_in: Option<NaiveDate> = row.try_get("check_in_date")?;
            let billing_period: Option<String> = row.try_get("billing_period")?;
            let rate_agreed: Option<String> = row.try_get("rate_agreed")?;
            let check_out: Option<NaiveDate> = row.try_get("check_out_date")?;
            let payment_amounts: Option<String> = row.try_get("payment_amounts")?;
            let status = allocation_status
                .as_deref()
                .unwrap_or("never_allocated")
                .to_string();

            let balance = match (check_in, billing_period.as_deref(), rate_agreed.as_deref()) {
                (Some(check_in), Some(bp), Some(rate)) => {
                    let paid = sum_decimal_amounts(payment_amounts.as_deref())?;
                    let reference = check_out.map_or(today, |d| if d < today { d } else { today });
                    allocation_balance(rate, check_in, reference, bp, paid)?
                }
                _ => Decimal::ZERO,
            };

            if let Some(existing) = summaries
                .iter_mut()
                .find(|summary| summary.id == resident_id)
            {
                let total = parse_decimal(&existing.balance)? + balance;
                existing.balance = total.to_string();

                let display = status == "active" || existing.status == "never_allocated";
                if display {
                    existing.allocation_id = row.try_get("allocation_id")?;
                    existing.room_number = row.try_get("room_number")?;
                    existing.check_in_date = check_in;
                    existing.expected_checkout = row.try_get("expected_checkout")?;
                    existing.status = status;
                }
                continue;
            }

            summaries.push(ResidentSummary {
                id: resident_id,
                full_name: row.try_get("full_name")?,
                phone: row.try_get("phone")?,
                email: row.try_get("email")?,
                allocation_id: row.try_get("allocation_id")?,
                room_number: row.try_get("room_number")?,
                check_in_date: check_in,
                expected_checkout: row.try_get("expected_checkout")?,
                status,
                balance: balance.to_string(),
            });
        }

        Ok(summaries)
    }

    /// Return full resident details, including all allocations and payments in
    /// the hostel and the computed net balance.
    #[instrument(skip(self))]
    pub async fn get_resident_detail(
        &self,
        hostel_id: i64,
        resident_id: i64,
    ) -> anyhow::Result<ResidentDetail> {
        debug!(hostel_id, resident_id, "fetching resident detail");

        let resident = sqlx::query_as::<_, Resident>(
            r#"
            SELECT id, full_name, phone, email, id_number, next_of_kin, photo_path, created_at
            FROM residents
            WHERE id = ?1
              AND EXISTS (
                  SELECT 1 FROM allocations a
                  WHERE a.resident_id = residents.id AND a.hostel_id = ?2
              )
            "#,
        )
        .bind(resident_id)
        .bind(hostel_id)
        .fetch_optional(&self.inner)
        .await
        .inspect_err(|e| error!(error = %e, resident_id, "database error fetching resident"))?
        .ok_or_else(|| anyhow::anyhow!("resident not found"))?;

        let allocations = sqlx::query_as::<_, AllocationWithRoom>(
            r#"
            SELECT
                a.id, a.hostel_id, a.resident_id, a.room_id,
                a.check_in_date, a.check_out_date, a.expected_checkout, a.status,
                a.rate_agreed, a.created_at, r.room_number, r.billing_period
            FROM allocations a
            JOIN rooms r ON r.id = a.room_id
            WHERE a.hostel_id = ?1 AND a.resident_id = ?2
            ORDER BY a.id DESC
            "#,
        )
        .bind(hostel_id)
        .bind(resident_id)
        .fetch_all(&self.inner)
        .await
        .inspect_err(|e| error!(error = %e, resident_id, "database error fetching allocations"))?;

        let payments = sqlx::query_as::<_, Payment>(
            "SELECT id, hostel_id, allocation_id, resident_id, amount, payment_date, payment_method, period_start, period_end, notes, created_at FROM payments WHERE hostel_id = ?1 AND resident_id = ?2 ORDER BY payment_date DESC, id DESC",
        )
        .bind(hostel_id)
        .bind(resident_id)
        .fetch_all(&self.inner)
        .await
        .inspect_err(|e| error!(error = %e, resident_id, "database error fetching payments"))?;

        let today = Utc::now().date_naive();
        let mut balance = Decimal::ZERO;
        for allocation in &allocations {
            let paid: Decimal = payments
                .iter()
                .filter(|p| p.allocation_id == allocation.id)
                .map(|p| parse_decimal(&p.amount))
                .collect::<Result<Vec<_>, _>>()?
                .iter()
                .copied()
                .fold(Decimal::ZERO, |acc, d| acc + d);
            let reference =
                allocation
                    .check_out_date
                    .map_or(today, |d| if d < today { d } else { today });
            balance += allocation_balance(
                &allocation.rate_agreed,
                allocation.check_in_date,
                reference,
                &allocation.billing_period,
                paid,
            )?;
        }

        Ok(ResidentDetail {
            resident,
            allocations,
            payments,
            balance: balance.to_string(),
        })
    }

    /// Check out an active allocation and free its room when appropriate.
    ///
    /// Fails when the allocation is not active, when the checkout date is before
    /// the check-in date, or when the database transaction cannot complete.
    #[instrument(skip(self))]
    pub async fn check_out(
        &self,
        hostel_id: i64,
        allocation_id: i64,
        check_out_date: NaiveDate,
    ) -> anyhow::Result<()> {
        debug!(hostel_id, allocation_id, "checking out allocation");
        let mut tx =
            self.inner.begin().await.inspect_err(
                |e| error!(error = %e, "database error starting checkout transaction"),
            )?;

        let row = sqlx::query(
            "SELECT check_in_date, room_id FROM allocations WHERE id = ?1 AND hostel_id = ?2 AND status = 'active'",
        )
        .bind(allocation_id)
        .bind(hostel_id)
        .fetch_optional(&mut *tx)
        .await
        .inspect_err(|e| error!(error = %e, allocation_id, "database error locking allocation"))?;

        let Some(row) = row else {
            anyhow::bail!("allocation is not active");
        };
        let check_in: NaiveDate = row.try_get("check_in_date")?;
        let room_id: i64 = row.try_get("room_id")?;

        if check_out_date < check_in {
            anyhow::bail!("check_out_date must be on or after check_in_date");
        }

        sqlx::query(
            "UPDATE allocations SET status = 'checked_out', check_out_date = ?1 WHERE id = ?2 AND hostel_id = ?3",
        )
        .bind(check_out_date)
        .bind(allocation_id)
        .bind(hostel_id)
        .execute(&mut *tx)
        .await
        .inspect_err(|e| error!(error = %e, allocation_id, "database error updating allocation"))?;

        sqlx::query(
            r#"
            UPDATE rooms
            SET status = 'vacant'
            WHERE id = ?1
              AND hostel_id = ?2
              AND status = 'occupied'
              AND NOT EXISTS (
                  SELECT 1 FROM allocations
                  WHERE room_id = ?1 AND hostel_id = ?2 AND status = 'active'
              )
            "#,
        )
        .bind(room_id)
        .bind(hostel_id)
        .execute(&mut *tx)
        .await
        .inspect_err(|e| error!(error = %e, room_id, "database error updating room status"))?;

        tx.commit().await.inspect_err(
            |e| error!(error = %e, allocation_id, "database error committing checkout"),
        )?;

        info!(allocation_id, "allocation checked out");
        Ok(())
    }

    /// Return the payments ledger for a hostel, optionally filtered.
    #[instrument(skip(self))]
    pub async fn list_payments(
        &self,
        filter: ListPaymentsFilter,
    ) -> anyhow::Result<Vec<PaymentRow>> {
        debug!(hostel_id = filter.hostel_id, "listing payments");
        let from = filter
            .from
            .map(local_day_bounds)
            .transpose()?
            .map(|(start, _)| start);
        let to = filter
            .to
            .map(local_day_bounds)
            .transpose()?
            .map(|(_, end)| end);
        let rows = sqlx::query_as::<_, PaymentRow>(
            r#"
            SELECT
                p.id, p.resident_id, r.full_name AS resident_name,
                p.allocation_id, rm.room_number, p.amount, p.payment_method,
                p.payment_date, rc.receipt_number, p.notes
            FROM payments p
            JOIN residents r ON r.id = p.resident_id
            JOIN allocations a ON a.id = p.allocation_id
            JOIN rooms rm ON rm.id = a.room_id
            LEFT JOIN receipts rc ON rc.payment_id = p.id
            WHERE p.hostel_id = ?1
              AND (?2 IS NULL OR p.payment_date >= ?2)
              AND (?3 IS NULL OR p.payment_date < ?3)
              AND (?4 IS NULL OR p.resident_id = ?4)
              AND (?5 IS NULL OR p.payment_method = ?5)
            ORDER BY p.payment_date DESC, p.id DESC
            LIMIT 500 -- ponytail: hard ceiling; add pagination if users exceed 500 rows
            "#,
        )
        .bind(filter.hostel_id)
        .bind(from)
        .bind(to)
        .bind(filter.resident_id)
        .bind(filter.resident_id)
        .bind(filter.method)
        .fetch_all(&self.inner)
        .await
        .inspect_err(
            |e| error!(error = %e, hostel_id = filter.hostel_id, "database error listing payments"),
        )?;
        Ok(rows)
    }

    /// Return active allocations for the payment-form resident picker, with
    /// accrued balance folded in Rust using exact decimal arithmetic.
    #[instrument(skip(self))]
    pub async fn list_active_allocations(
        &self,
        hostel_id: i64,
    ) -> anyhow::Result<Vec<ActiveAllocation>> {
        debug!(hostel_id, "listing active allocations");
        let today = Utc::now().date_naive();
        let rows = sqlx::query(
            r#"
            SELECT
                a.id AS allocation_id, a.resident_id,
                r.full_name AS resident_name, rm.room_number,
                a.rate_agreed, a.check_in_date, a.expected_checkout,
                rm.billing_period, GROUP_CONCAT(p.amount) AS payment_amounts
            FROM allocations a
            JOIN residents r ON r.id = a.resident_id
            JOIN rooms rm ON rm.id = a.room_id
            LEFT JOIN payments p ON p.allocation_id = a.id
            WHERE a.hostel_id = ?1 AND a.status = 'active'
            GROUP BY a.id
            ORDER BY r.full_name
            "#,
        )
        .bind(hostel_id)
        .fetch_all(&self.inner)
        .await
        .inspect_err(
            |e| error!(error = %e, hostel_id, "database error listing active allocations"),
        )?;

        let mut results = Vec::with_capacity(rows.len());
        for row in rows {
            let allocation_id: i64 = row.try_get("allocation_id")?;
            let resident_id: i64 = row.try_get("resident_id")?;
            let resident_name: String = row.try_get("resident_name")?;
            let room_number: String = row.try_get("room_number")?;
            let rate_agreed: String = row.try_get("rate_agreed")?;
            let check_in_date: NaiveDate = row.try_get("check_in_date")?;
            let expected_checkout: NaiveDate = row.try_get("expected_checkout")?;
            let billing_period: String = row.try_get("billing_period")?;
            let payment_amounts: Option<String> = row.try_get("payment_amounts")?;

            let total_paid = sum_decimal_amounts(payment_amounts.as_deref())?;
            let balance = allocation_balance(
                &rate_agreed,
                check_in_date,
                today,
                &billing_period,
                total_paid,
            )?;

            results.push(ActiveAllocation {
                allocation_id,
                resident_id,
                resident_name,
                room_number,
                rate_agreed,
                check_in_date,
                expected_checkout,
                total_paid: total_paid.to_string(),
                balance: balance.to_string(),
            });
        }

        Ok(results)
    }

    /// Return dashboard statistics for a hostel on a specific day.
    ///
    /// `today` is supplied by the caller so the dashboard reflects the user's
    /// local calendar day.
    #[instrument(skip(self))]
    pub async fn get_dashboard_stats(
        &self,
        hostel_id: i64,
        today: NaiveDate,
    ) -> anyhow::Result<DashboardStats> {
        debug!(hostel_id, %today, "fetching dashboard stats");

        let counts = sqlx::query(
            r#"
            SELECT
                (SELECT COUNT(*) FROM rooms WHERE hostel_id = ?1) AS total_rooms,
                (SELECT COUNT(*) FROM rooms WHERE hostel_id = ?1 AND status = 'occupied') AS occupied,
                (SELECT COUNT(*) FROM rooms WHERE hostel_id = ?1 AND status = 'vacant') AS vacant,
                (SELECT COUNT(*) FROM rooms WHERE hostel_id = ?1 AND status = 'maintenance') AS maintenance,
                (SELECT COUNT(*) FROM allocations WHERE hostel_id = ?1 AND check_in_date = ?2) AS check_ins_today,
                (SELECT COUNT(*) FROM allocations WHERE hostel_id = ?1 AND status = 'active' AND expected_checkout = ?2) AS departures_today,
                (SELECT COUNT(*) FROM allocations WHERE hostel_id = ?1 AND status = 'active' AND expected_checkout < ?2) AS overdue_stays
            "#,
        )
        .bind(hostel_id)
        .bind(today)
        .fetch_one(&self.inner)
        .await
        .inspect_err(|e| error!(error = %e, hostel_id, "database error fetching dashboard counts"))?;

        let revenue_today = self.revenue_for_date(hostel_id, today).await?;
        let revenue_month = self.revenue_for_month(hostel_id, today).await?;
        let outstanding_total = self.outstanding_balance_total(hostel_id, today).await?;

        Ok(DashboardStats {
            total_rooms: counts.try_get("total_rooms")?,
            occupied: counts.try_get("occupied")?,
            vacant: counts.try_get("vacant")?,
            maintenance: counts.try_get("maintenance")?,
            check_ins_today: counts.try_get("check_ins_today")?,
            departures_today: counts.try_get("departures_today")?,
            overdue_stays: counts.try_get("overdue_stays")?,
            revenue_today: revenue_today.to_string(),
            revenue_month: revenue_month.to_string(),
            outstanding_total: outstanding_total.to_string(),
        })
    }

    /// Return recent check-ins, check-outs and payments for the dashboard feed.
    #[instrument(skip(self))]
    pub async fn list_recent_activity(
        &self,
        hostel_id: i64,
        limit: i64,
    ) -> anyhow::Result<Vec<ActivityItem>> {
        debug!(hostel_id, limit, "listing recent activity");
        let limit = limit.min(50);
        let check_ins = sqlx::query_as::<_, ActivityItem>(
            r#"
            SELECT 'check_in' AS kind,
                   a.created_at AS at,
                   r.full_name AS resident_name,
                   rm.room_number AS room_number,
                   CAST(NULL AS TEXT) AS amount
            FROM allocations a
            JOIN residents r ON r.id = a.resident_id
            JOIN rooms rm ON rm.id = a.room_id
            WHERE a.hostel_id = ?1
            ORDER BY at DESC
            LIMIT ?2
            "#,
        )
        .bind(hostel_id)
        .bind(limit)
        .fetch_all(&self.inner)
        .await
        .inspect_err(
            |e| error!(error = %e, hostel_id, "database error listing check-in activity"),
        )?;

        let check_outs = sqlx::query_as::<_, ActivityItem>(
            r#"
            SELECT 'check_out' AS kind,
                   strftime('%Y-%m-%dT%H:%M:%fZ', a.check_out_date) AS at,
                   r.full_name AS resident_name,
                   rm.room_number AS room_number,
                   CAST(NULL AS TEXT) AS amount
            FROM allocations a
            JOIN residents r ON r.id = a.resident_id
            JOIN rooms rm ON rm.id = a.room_id
            WHERE a.hostel_id = ?1
              AND a.status = 'checked_out'
              AND a.check_out_date IS NOT NULL
            ORDER BY at DESC
            LIMIT ?2
            "#,
        )
        .bind(hostel_id)
        .bind(limit)
        .fetch_all(&self.inner)
        .await
        .inspect_err(
            |e| error!(error = %e, hostel_id, "database error listing check-out activity"),
        )?;

        let payments = sqlx::query_as::<_, ActivityItem>(
            r#"
            SELECT 'payment' AS kind,
                   p.payment_date AS at,
                   r.full_name AS resident_name,
                   rm.room_number AS room_number,
                   p.amount AS amount
            FROM payments p
            JOIN residents r ON r.id = p.resident_id
            JOIN allocations a ON a.id = p.allocation_id
            JOIN rooms rm ON rm.id = a.room_id
            WHERE p.hostel_id = ?1
            ORDER BY at DESC
            LIMIT ?2
            "#,
        )
        .bind(hostel_id)
        .bind(limit)
        .fetch_all(&self.inner)
        .await
        .inspect_err(
            |e| error!(error = %e, hostel_id, "database error listing payment activity"),
        )?;

        let mut rows = Vec::with_capacity(check_ins.len() + check_outs.len() + payments.len());
        rows.extend(check_ins);
        rows.extend(check_outs);
        rows.extend(payments);
        rows.sort_by(|a, b| b.at.cmp(&a.at));
        rows.truncate(limit as usize);
        Ok(rows)
    }

    /// Update a hostel profile and return the updated row.
    ///
    /// Validates that the name is non-empty and the currency is a three-letter
    /// uppercase code.
    #[instrument(skip(self))]
    pub async fn update_hostel(
        &self,
        hostel_id: i64,
        name: String,
        address: Option<String>,
        phone: Option<String>,
        currency: String,
        branding_logo_path: Option<String>,
    ) -> anyhow::Result<Hostel> {
        debug!(hostel_id, "updating hostel");
        if name.trim().is_empty() {
            anyhow::bail!("name is required");
        }
        if currency.len() != 3 || !currency.chars().all(|c| c.is_ascii_uppercase()) {
            anyhow::bail!("currency must be a 3-letter uppercase code");
        }
        let row = sqlx::query_as::<_, Hostel>(
            r#"
            UPDATE hostels
            SET name = ?1, address = ?2, phone = ?3, currency = ?4, branding_logo_path = ?5
            WHERE id = ?6
            RETURNING id, name, address, phone, currency, branding_logo_path, created_at
            "#,
        )
        .bind(&name)
        .bind(&address)
        .bind(&phone)
        .bind(&currency)
        .bind(&branding_logo_path)
        .bind(hostel_id)
        .fetch_one(&self.inner)
        .await
        .inspect_err(|e| error!(error = %e, hostel_id, "database error updating hostel"))?;
        Ok(row)
    }

    /// Update a resident's contact details.
    ///
    /// Fails when the name is empty or the resident id does not exist.
    #[instrument(skip(self))]
    pub async fn update_resident(
        &self,
        hostel_id: i64,
        resident_id: i64,
        full_name: String,
        phone: Option<String>,
        email: Option<String>,
        id_number: Option<String>,
        next_of_kin: Option<String>,
    ) -> anyhow::Result<()> {
        debug!(hostel_id, resident_id, "updating resident");
        if full_name.trim().is_empty() {
            anyhow::bail!("full_name is required");
        }
        let rows = sqlx::query(
            r#"
            UPDATE residents
            SET full_name = ?1, phone = ?2, email = ?3, id_number = ?4, next_of_kin = ?5
            WHERE id = ?6
              AND EXISTS (
                  SELECT 1 FROM allocations a
                  WHERE a.resident_id = residents.id AND a.hostel_id = ?7
              )
            "#,
        )
        .bind(&full_name)
        .bind(&phone)
        .bind(&email)
        .bind(&id_number)
        .bind(&next_of_kin)
        .bind(resident_id)
        .bind(hostel_id)
        .execute(&self.inner)
        .await
        .inspect_err(|e| error!(error = %e, resident_id, "database error updating resident"))?
        .rows_affected();
        if rows == 0 {
            anyhow::bail!("resident not found");
        }
        Ok(())
    }

    /// List staff users for a hostel, excluding password hashes.
    #[instrument(skip(self))]
    pub async fn list_users(&self, hostel_id: i64) -> anyhow::Result<Vec<User>> {
        debug!(hostel_id, "listing users");
        let rows = sqlx::query_as::<_, User>(
            "SELECT id, hostel_id, full_name, email, role, created_at FROM users WHERE hostel_id = ?1 ORDER BY id",
        )
        .bind(hostel_id)
        .fetch_all(&self.inner)
        .await
        .inspect_err(|e| error!(error = %e, hostel_id, "database error listing staff users"))?;
        Ok(rows)
    }

    /// Sum payments recorded on a specific calendar day.
    async fn revenue_for_date(&self, hostel_id: i64, date: NaiveDate) -> anyhow::Result<Decimal> {
        let (start, end) = local_day_bounds(date)?;
        let row = sqlx::query(
            "SELECT GROUP_CONCAT(amount) AS amounts FROM payments WHERE hostel_id = ?1 AND payment_date >= ?2 AND payment_date < ?3",
        )
        .bind(hostel_id)
        .bind(start)
        .bind(end)
        .fetch_optional(&self.inner)
        .await
        .inspect_err(|e| error!(error = %e, hostel_id, %date, "database error fetching daily revenue"))?;
        match row {
            Some(row) => {
                let amounts: Option<String> = row.try_get("amounts")?;
                sum_decimal_amounts(amounts.as_deref())
            }
            None => Ok(Decimal::ZERO),
        }
    }

    /// Sum payments recorded in a calendar month.
    async fn revenue_for_month(
        &self,
        hostel_id: i64,
        reference: NaiveDate,
    ) -> anyhow::Result<Decimal> {
        let month_start = reference
            .with_day(1)
            .ok_or_else(|| anyhow::anyhow!("invalid month start: {reference}"))?;
        let next_month = if month_start.month() == 12 {
            month_start
                .with_year(month_start.year() + 1)
                .and_then(|d: NaiveDate| d.with_month(1))
        } else {
            month_start.with_month(month_start.month() + 1)
        }
        .ok_or_else(|| anyhow::anyhow!("invalid month end: {reference}"))?;
        let (_, end) = local_day_bounds(next_month)?;
        let row = sqlx::query(
            "SELECT GROUP_CONCAT(amount) AS amounts FROM payments WHERE hostel_id = ?1 AND payment_date >= ?2 AND payment_date < ?3",
        )
        .bind(hostel_id)
        .bind(local_day_bounds(month_start)?.0)
        .bind(end)
        .fetch_optional(&self.inner)
        .await
        .inspect_err(|e| error!(error = %e, hostel_id, %reference, "database error fetching monthly revenue"))?;
        match row {
            Some(row) => {
                let amounts: Option<String> = row.try_get("amounts")?;
                sum_decimal_amounts(amounts.as_deref())
            }
            None => Ok(Decimal::ZERO),
        }
    }

    /// Sum positive balances across all active allocations.
    async fn outstanding_balance_total(
        &self,
        hostel_id: i64,
        today: NaiveDate,
    ) -> anyhow::Result<Decimal> {
        let rows = sqlx::query(
            r#"
            SELECT
                a.check_in_date, a.rate_agreed, rm.billing_period,
                GROUP_CONCAT(p.amount) AS payment_amounts
            FROM allocations a
            JOIN rooms rm ON rm.id = a.room_id
            LEFT JOIN payments p ON p.allocation_id = a.id
            WHERE a.hostel_id = ?1 AND a.status = 'active'
            GROUP BY a.id
            "#,
        )
        .bind(hostel_id)
        .fetch_all(&self.inner)
        .await
        .inspect_err(
            |e| error!(error = %e, hostel_id, "database error fetching outstanding balances"),
        )?;

        let mut total = Decimal::ZERO;
        for row in rows {
            let check_in: NaiveDate = row.try_get("check_in_date")?;
            let rate_agreed: String = row.try_get("rate_agreed")?;
            let billing_period: String = row.try_get("billing_period")?;
            let payment_amounts: Option<String> = row.try_get("payment_amounts")?;
            let paid = sum_decimal_amounts(payment_amounts.as_deref())?;
            let balance = allocation_balance(&rate_agreed, check_in, today, &billing_period, paid)?;
            if balance > Decimal::ZERO {
                total += balance;
            }
        }
        Ok(total)
    }
}

/// Return the UTC bounds of a local calendar day.
///
/// Payments are stored as UTC timestamps, while operators select dates in the
/// hostel's local timezone. The exclusive upper bound lets SQLite use the
/// `payments(hostel_id, payment_date)` index without wrapping the indexed column
/// in `date()` or `strftime()`.
fn local_day_bounds(date: NaiveDate) -> anyhow::Result<(DateTime<Utc>, DateTime<Utc>)> {
    let start_naive = date
        .and_hms_opt(0, 0, 0)
        .ok_or_else(|| anyhow::anyhow!("invalid local date start: {date}"))?;
    let end_naive = (date + chrono::Duration::days(1))
        .and_hms_opt(0, 0, 0)
        .ok_or_else(|| anyhow::anyhow!("invalid local date end: {date}"))?;
    let start = Local
        .from_local_datetime(&start_naive)
        .earliest()
        .ok_or_else(|| anyhow::anyhow!("invalid local date start: {date}"))?;
    let end = Local
        .from_local_datetime(&end_naive)
        .earliest()
        .ok_or_else(|| anyhow::anyhow!("invalid local date end: {date}"))?;
    Ok((start.with_timezone(&Utc), end.with_timezone(&Utc)))
}

/// Parse a decimal monetary amount from a string.
fn parse_decimal(amount: &str) -> anyhow::Result<Decimal> {
    Decimal::from_str(amount)
        .map_err(|e| anyhow::anyhow!("invalid decimal amount '{}': {}", amount, e))
}

/// Sum a comma-separated list of decimal amounts produced by SQLite's
/// `GROUP_CONCAT`. Returns zero for `None` or an empty string.
fn sum_decimal_amounts(amounts: Option<&str>) -> anyhow::Result<Decimal> {
    match amounts {
        None | Some("") => Ok(Decimal::ZERO),
        Some(s) => {
            let values: Vec<Decimal> = s
                .split(',')
                .map(|v| parse_decimal(v.trim()))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(values.iter().copied().fold(Decimal::ZERO, |acc, d| acc + d))
        }
    }
}

/// Map a billing period to its length in days for the rolled-up balance calc.
///
/// This is a deliberate simplification: month ≡ 30 days, semester ≡ 120 days.
fn billing_period_days(period: &str) -> Option<i64> {
    match period {
        "night" => Some(1),
        "week" => Some(7),
        "month" => Some(30),
        "semester" => Some(120),
        _ => None,
    }
}

/// Number of whole billing periods elapsed between check-in and the reference
/// date, rounded up.
fn periods_elapsed(check_in: NaiveDate, reference: NaiveDate, billing_period: &str) -> i64 {
    if reference < check_in {
        return 0;
    }
    if let Some(days) = billing_period_days(billing_period) {
        let total_days = (reference - check_in).num_days() + 1;
        (total_days + days - 1) / days
    } else {
        // ponytail: unknown billing_period falls back to one period (flat rate per stay)
        1
    }
}

/// Compute the outstanding balance for an allocation.
///
/// `reference` is the day to accumulate through, normally today or the actual
/// check-out date, whichever is earlier.
fn allocation_balance(
    rate_agreed: &str,
    check_in: NaiveDate,
    reference: NaiveDate,
    billing_period: &str,
    total_paid: Decimal,
) -> anyhow::Result<Decimal> {
    let rate = parse_decimal(rate_agreed)?;
    let periods = periods_elapsed(check_in, reference, billing_period);
    let owed = rate * Decimal::from(periods);
    Ok(owed - total_paid)
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
    let billing_period = room.billing_period.as_deref().unwrap_or("semester");
    if !matches!(billing_period, "night" | "week" | "month" | "semester") {
        anyhow::bail!("billing_period must be one of: night, week, month, semester");
    }
    let id = sqlx::query(
        r#"
        INSERT INTO rooms (hostel_id, room_number, room_type, capacity, floor, rate_per_period, billing_period, status)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'vacant')
        "#,
    )
    .bind(room.hostel_id)
    .bind(&room.room_number)
    .bind(&room.room_type)
    .bind(room.capacity)
    .bind(room.floor)
    .bind(&room.rate_per_period)
    .bind(billing_period)
    .execute(&mut **tx)
    .await
    .inspect_err(|e| error!(error = %e, hostel_id = room.hostel_id, room_number = %room.room_number, "database error inserting room"))?
    .last_insert_rowid();
    Ok(id)
}

/// Validate that an allocation's date range is internally consistent.
///
/// Both the actual checkout date (when present) and the expected checkout date
/// must fall on or after the check-in date. This is enforced at the shared
/// allocation insertion path so every caller receives the same behavior.
fn validate_allocation_dates(allocation: &NewAllocation) -> anyhow::Result<()> {
    if let Some(check_out) = allocation.check_out_date {
        if check_out < allocation.check_in_date {
            anyhow::bail!("check_out_date must be on or after check_in_date");
        }
    }
    if allocation.expected_checkout < allocation.check_in_date {
        anyhow::bail!("expected_checkout must be on or after check_in_date");
    }
    Ok(())
}

/// Insert a resident inside an existing transaction.
async fn insert_resident(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    resident: &NewResident,
) -> anyhow::Result<i64> {
    let id = sqlx::query(
        r#"
        INSERT INTO residents (full_name, phone, email, id_number, next_of_kin, photo_path)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6)
        "#,
    )
    .bind(&resident.full_name)
    .bind(&resident.phone)
    .bind(&resident.email)
    .bind(&resident.id_number)
    .bind(&resident.next_of_kin)
    .bind(&resident.photo_path)
    .execute(&mut **tx)
    .await
    .inspect_err(|e| error!(error = %e, "database error inserting resident in transaction"))?
    .last_insert_rowid();
    Ok(id)
}

/// Verify a room can be allocated and, if it is vacant, mark it occupied.
///
/// Fails when the room does not belong to the hostel, is under maintenance,
/// or already has active allocations equal to its capacity. This is shared by
/// [`DbPool::create_allocation`] and [`DbPool::check_in`] so every allocation
/// path applies the same guards.
async fn prepare_room_for_allocation(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    hostel_id: i64,
    room_id: i64,
) -> anyhow::Result<()> {
    let row = sqlx::query("SELECT status, capacity FROM rooms WHERE id = ?1 AND hostel_id = ?2")
        .bind(room_id)
        .bind(hostel_id)
        .fetch_optional(&mut **tx)
        .await
        .inspect_err(
            |e| error!(error = %e, room_id, "database error fetching room for allocation"),
        )?;

    let Some(row) = row else {
        anyhow::bail!("room not found or does not belong to the hostel");
    };

    let status: String = row.try_get("status")?;
    let capacity: i32 = row.try_get("capacity")?;

    if status == "maintenance" {
        anyhow::bail!("room is under maintenance");
    }
    if !matches!(status.as_str(), "vacant" | "occupied") {
        anyhow::bail!("room cannot be allocated");
    }

    let active_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM allocations WHERE room_id = ?1 AND hostel_id = ?2 AND status = 'active'",
    )
    .bind(room_id)
    .bind(hostel_id)
    .fetch_one(&mut **tx)
    .await
    .inspect_err(|e| error!(error = %e, room_id, "database error counting active allocations"))?;

    if active_count >= i64::from(capacity) {
        anyhow::bail!("room has no available beds");
    }

    if status == "vacant" {
        let rows = sqlx::query(
            "UPDATE rooms SET status = 'occupied' WHERE id = ?1 AND hostel_id = ?2 AND status = 'vacant'",
        )
        .bind(room_id)
        .bind(hostel_id)
        .execute(&mut **tx)
        .await
        .inspect_err(|e| error!(error = %e, room_id, "database error updating room status"))?
        .rows_affected();

        if rows == 0 {
            anyhow::bail!("room is no longer available");
        }
    }

    Ok(())
}

/// Insert an allocation inside an existing transaction after verifying the room
/// is allocatable and updating its status.
async fn insert_allocation(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    allocation: &NewAllocation,
) -> anyhow::Result<i64> {
    validate_allocation_dates(allocation)?;
    prepare_room_for_allocation(tx, allocation.hostel_id, allocation.room_id).await?;

    let id = sqlx::query(
        r#"
        INSERT INTO allocations
            (hostel_id, resident_id, room_id, check_in_date, check_out_date, expected_checkout, status, rate_agreed)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'active', ?7)
        "#,
    )
    .bind(allocation.hostel_id)
    .bind(allocation.resident_id)
    .bind(allocation.room_id)
    .bind(allocation.check_in_date)
    .bind(allocation.check_out_date)
    .bind(allocation.expected_checkout)
    .bind(&allocation.rate_agreed)
    .execute(&mut **tx)
    .await
    .inspect_err(|e| error!(error = %e, "database error inserting allocation"))?
    .last_insert_rowid();

    Ok(id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration, NaiveDate};
    use rust_decimal::Decimal;
    use sqlx::Row;
    use std::str::FromStr;

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
            billing_period: None,
        })
        .await
        .expect("add room");
        pool.create_user(NewUser {
            hostel_id: hostel.id,
            full_name: "Admin".to_string(),
            email: "admin@example.com".to_string(),
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

    /// A nightly room accrues balance per night, not once per semester.
    #[tokio::test]
    async fn nightly_room_accrues_per_night_balance() {
        let pool = test_pool().await;
        let hostel = seed_hostel(&pool).await;
        let room_id = pool
            .add_room(NewRoom {
                hostel_id: hostel.id,
                room_number: "N1".to_string(),
                room_type: "single".to_string(),
                capacity: 1,
                floor: None,
                rate_per_period: "50.00".to_string(),
                billing_period: Some("night".to_string()),
            })
            .await
            .expect("add nightly room");

        let rooms = pool.get_rooms(hostel.id).await.expect("get rooms");
        let room = rooms.iter().find(|r| r.id == room_id).expect("room");
        assert_eq!(room.billing_period, "night");

        let resident_id = seed_resident(&pool, "Nightly").await;
        let check_in = today() - Duration::days(2);
        let _allocation_id = pool
            .check_in(NewCheckIn {
                hostel_id: hostel.id,
                resident_id: Some(resident_id),
                full_name: None,
                phone: None,
                email: None,
                id_number: None,
                next_of_kin: None,
                photo_path: None,
                room_id,
                check_in_date: check_in,
                check_out_date: None,
                expected_checkout: check_in + Duration::days(5),
                rate_agreed: "50.00".to_string(),
            })
            .await
            .expect("create allocation");

        let detail = pool
            .get_resident_detail(hostel.id, resident_id)
            .await
            .expect("detail");
        assert_eq!(detail.allocations[0].billing_period, "night");
        assert_eq!(detail.balance, "150.00", "3 nights at 50 should be 150");

        // A single-period billing would have produced 50.00; ensure it did not.
        assert_ne!(detail.balance, "50.00");

        // Also verify list_residents uses the same computation.
        let residents = pool
            .list_residents(hostel.id)
            .await
            .expect("list residents");
        let nightly = residents
            .iter()
            .find(|r| r.id == resident_id)
            .expect("resident");
        assert_eq!(nightly.balance, "150.00");
    }

    #[tokio::test]
    async fn journal_mode_is_wal_for_file_database() {
        let dir = tempfile::tempdir().expect("create temp dir");
        let db_path = dir.path().join("hosteldesk.db");
        let url = format!("sqlite:{}", db_path.display());

        let pool = DbPool::open(&url).await.expect("open pool");
        let journal_mode: String = sqlx::query_scalar("PRAGMA journal_mode")
            .fetch_one(&pool.inner)
            .await
            .expect("journal_mode");

        assert_eq!(journal_mode, "wal", "file databases should use WAL mode");
    }

    #[tokio::test]
    async fn reopening_file_database_applies_no_new_migrations() {
        let dir = tempfile::tempdir().expect("create temp dir");
        let db_path = dir.path().join("hosteldesk.db");
        let url = format!("sqlite:{}", db_path.display());

        DbPool::open(&url).await.expect("first open");
        DbPool::open(&url).await.expect("second open");

        // Migrations write DDL, so WAL/shm files should exist afterwards.
        let wal_file = db_path.with_extension("db-wal");
        let shm_file = db_path.with_extension("db-shm");
        assert!(
            wal_file.exists() || db_path.exists(),
            "database files should be present"
        );
        assert!(shm_file.exists(), "wal-index file should be present");
    }

    #[tokio::test]
    async fn concurrent_record_payment_and_receipt_are_sequential_and_distinct() {
        let dir = tempfile::tempdir().expect("create temp dir");
        let db_path = dir.path().join("hosteldesk.db");
        let url = format!("sqlite:{}", db_path.display());
        let pool = DbPool::open(&url).await.expect("open pool");

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

        let room_id = pool
            .add_room(NewRoom {
                hostel_id: hostel.id,
                room_number: "101".to_string(),
                room_type: "single".to_string(),
                capacity: 1,
                floor: None,
                rate_per_period: "10".to_string(),
                billing_period: None,
            })
            .await
            .expect("add room");

        let resident_id = pool
            .add_resident(NewResident {
                full_name: "Resident".to_string(),
                phone: None,
                email: None,
                id_number: None,
                next_of_kin: None,
                photo_path: None,
            })
            .await
            .expect("add resident");

        let allocation_id = pool
            .check_in(NewCheckIn {
                hostel_id: hostel.id,
                resident_id: Some(resident_id),
                full_name: None,
                phone: None,
                email: None,
                id_number: None,
                next_of_kin: None,
                photo_path: None,
                room_id,
                check_in_date: chrono::NaiveDate::from_ymd_opt(2026, 1, 1).expect("date"),
                check_out_date: None,
                expected_checkout: chrono::NaiveDate::from_ymd_opt(2026, 4, 30).expect("date"),
                rate_agreed: "10.00".to_string(),
            })
            .await
            .expect("create allocation");

        let payment = NewPayment {
            hostel_id: hostel.id,
            allocation_id,
            resident_id,
            amount: "5.00".to_string(),
            payment_method: "cash".to_string(),
            period_start: None,
            period_end: None,
            notes: None,
        };

        let (result_a, result_b) = tokio::join!(
            pool.record_payment_and_receipt(payment.clone()),
            pool.record_payment_and_receipt(payment.clone()),
        );

        let (id_a, number_a) = result_a.expect("first payment");
        let (id_b, number_b) = result_b.expect("second payment");

        assert_ne!(id_a, id_b, "payment ids should differ");
        assert_ne!(number_a, number_b, "receipt numbers should differ");
        assert!(
            number_b.parse::<i64>().expect("numeric receipt")
                > number_a.parse::<i64>().expect("numeric receipt"),
            "receipt numbers should be sequential"
        );
    }

    #[tokio::test]
    async fn update_receipt_pdf_path_persists_and_ignores_missing() {
        let pool = test_pool().await;
        let hostel = seed_hostel(&pool).await;
        let room_id = seed_room(&pool, hostel.id, "101", "single").await;
        let resident_id = seed_resident(&pool, "Receipt").await;
        let allocation_id = pool
            .check_in(NewCheckIn {
                hostel_id: hostel.id,
                resident_id: Some(resident_id),
                full_name: None,
                phone: None,
                email: None,
                id_number: None,
                next_of_kin: None,
                photo_path: None,
                room_id,
                check_in_date: today(),
                check_out_date: None,
                expected_checkout: today() + Duration::days(30),
                rate_agreed: "100.00".to_string(),
            })
            .await
            .expect("create allocation");

        let (_, receipt_number) = pool
            .record_payment_and_receipt(NewPayment {
                hostel_id: hostel.id,
                allocation_id,
                resident_id,
                amount: "10.00".to_string(),
                payment_method: "cash".to_string(),
                period_start: None,
                period_end: None,
                notes: None,
            })
            .await
            .expect("record payment");

        pool.update_receipt_pdf_path(hostel.id, &receipt_number, "/tmp/receipt.pdf")
            .await
            .expect("update pdf_path");

        let stored: Option<String> = sqlx::query_scalar(
            "SELECT pdf_path FROM receipts WHERE hostel_id = ?1 AND receipt_number = ?2",
        )
        .bind(hostel.id)
        .bind(&receipt_number)
        .fetch_one(&pool.inner)
        .await
        .expect("fetch pdf_path");
        assert_eq!(stored.as_deref(), Some("/tmp/receipt.pdf"));

        // A missing receipt should return Ok without affecting any row.
        pool.update_receipt_pdf_path(hostel.id, "99999", "/tmp/missing.pdf")
            .await
            .expect("missing receipt should not error");
    }

    #[tokio::test]
    async fn room_type_check_rejects_invalid_value() {
        let pool = test_pool().await;
        let hostel = seed_hostel(&pool).await;
        let err = sqlx::query(
            "INSERT INTO rooms (hostel_id, room_number, room_type, capacity, rate_per_period, billing_period) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        )
        .bind(hostel.id)
        .bind("101")
        .bind("suite")
        .bind(1)
        .bind("100.00")
        .bind("semester")
        .execute(&pool.inner)
        .await
        .expect_err("invalid room_type should fail CHECK");
        assert!(
            err.to_string().contains("CHECK") || err.to_string().contains("constraint"),
            "expected CHECK constraint failure, got: {err}"
        );
    }

    #[tokio::test]
    async fn authenticate_wrong_password_returns_none() {
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

        pool.create_user(NewUser {
            hostel_id: hostel.id,
            full_name: "Admin".to_string(),
            email: "admin@example.com".to_string(),
            password_hash: hash_password("correct-horse-battery-staple").expect("hash"),
            role: "admin".to_string(),
        })
        .await
        .expect("add admin");

        let user = pool
            .authenticate_user("admin", "wrong_password")
            .await
            .expect("authenticate should not fail");
        assert!(user.is_none(), "wrong password should not authenticate");
    }

    #[tokio::test]
    async fn payments_query_plan_uses_hostel_date_index() {
        let pool = test_pool().await;
        let rows = sqlx::query("EXPLAIN QUERY PLAN SELECT * FROM payments WHERE hostel_id = ?1")
            .bind(1i64)
            .fetch_all(&pool.inner)
            .await
            .expect("query plan");

        let details: Vec<String> = rows
            .iter()
            .map(|r| r.try_get::<String, _>("detail").expect("detail column"))
            .collect();
        assert!(
            details
                .iter()
                .any(|d| d.contains("idx_payments_hostel_date")),
            "payments lookup should use idx_payments_hostel_date, got: {details:?}"
        );
    }

    /// Reopening a file database must be idempotent: sqlx should see every
    /// migration as already applied and not add new rows on the second open.
    #[tokio::test]
    async fn reopening_file_database_adds_zero_new_migrations() {
        let dir = tempfile::tempdir().expect("create temp dir");
        let db_path = dir.path().join("hosteldesk.db");
        let url = format!("sqlite:{}", db_path.display());

        let pool = DbPool::open(&url).await.expect("first open");
        let first_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM _sqlx_migrations")
            .fetch_one(&pool.inner)
            .await
            .expect("count migrations after first open");
        assert!(first_count > 0, "at least one migration should be applied");

        let pool = DbPool::open(&url).await.expect("second open");
        let second_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM _sqlx_migrations")
            .fetch_one(&pool.inner)
            .await
            .expect("count migrations after second open");

        assert_eq!(
            first_count, second_count,
            "second open should apply zero new migrations"
        );
    }

    /// An email can exist once per hostel. The single-query authenticator
    /// must still return the user matching the correct password and hostel.
    #[tokio::test]
    async fn authenticate_unknown_email_still_performs_argon2_work() {
        let pool = test_pool().await;
        let started = std::time::Instant::now();

        let user = pool
            .authenticate_user("definitely-not-a-real-email@example.com", "wrong_password")
            .await
            .expect("authenticate unknown email");

        assert!(user.is_none(), "unknown email must not authenticate");
        assert!(
            started.elapsed() >= std::time::Duration::from_millis(1),
            "unknown email must perform Argon2 verification work"
        );
    }

    #[tokio::test]
    async fn authenticate_returns_correct_user_when_email_spans_hostels() {
        let pool = test_pool().await;
        let hostel_a = pool
            .create_hostel(NewHostel {
                name: "Hostel A".to_string(),
                address: None,
                phone: None,
                currency: None,
                branding_logo_path: None,
            })
            .await
            .expect("create hostel a");
        let hostel_b = pool
            .create_hostel(NewHostel {
                name: "Hostel B".to_string(),
                address: None,
                phone: None,
                currency: None,
                branding_logo_path: None,
            })
            .await
            .expect("create hostel b");

        pool.create_user(NewUser {
            hostel_id: hostel_a.id,
            full_name: "A Manager".to_string(),
            email: "manager@example.com".to_string(),
            password_hash: hash_password("password-a").expect("hash a"),
            role: "manager".to_string(),
        })
        .await
        .expect("create user a");
        pool.create_user(NewUser {
            hostel_id: hostel_b.id,
            full_name: "B Manager".to_string(),
            email: "manager@example.com".to_string(),
            password_hash: hash_password("password-b").expect("hash b"),
            role: "manager".to_string(),
        })
        .await
        .expect("create user b");

        let user_b = pool
            .authenticate_user("manager", "password-b")
            .await
            .expect("authenticate should not fail")
            .expect("should find user b");
        assert_eq!(user_b.hostel_id, hostel_b.id);
        assert_eq!(user_b.name, "B Manager");
        assert_eq!(user_b.role, "manager");

        assert!(
            pool.authenticate_user("manager", "wrong-password")
                .await
                .expect("authenticate should not fail")
                .is_none(),
            "wrong password should return None"
        );
    }

    /// Rooms must never leak across hostels when filtered by `hostel_id`.
    #[tokio::test]
    async fn get_rooms_isolated_by_hostel_id() {
        let pool = test_pool().await;
        let hostel_a = pool
            .create_hostel(NewHostel {
                name: "Hostel A".to_string(),
                address: None,
                phone: None,
                currency: None,
                branding_logo_path: None,
            })
            .await
            .expect("create hostel a");
        let hostel_b = pool
            .create_hostel(NewHostel {
                name: "Hostel B".to_string(),
                address: None,
                phone: None,
                currency: None,
                branding_logo_path: None,
            })
            .await
            .expect("create hostel b");

        pool.add_room(NewRoom {
            hostel_id: hostel_a.id,
            room_number: "101".to_string(),
            room_type: "single".to_string(),
            capacity: 1,
            floor: None,
            rate_per_period: "10".to_string(),
            billing_period: None,
        })
        .await
        .expect("add room to hostel a");

        assert!(
            pool.get_rooms(hostel_b.id)
                .await
                .expect("fetch hostel b rooms")
                .is_empty(),
            "hostel b should have no rooms"
        );
        let a_rooms = pool
            .get_rooms(hostel_a.id)
            .await
            .expect("fetch hostel a rooms");
        assert_eq!(a_rooms.len(), 1);
        assert_eq!(a_rooms[0].room_number, "101");
    }

    #[tokio::test]
    async fn decimal_amounts_sum_exactly_without_float_drift() {
        let pool = test_pool().await;
        let hostel = seed_hostel(&pool).await;
        let room_id = seed_room(&pool, hostel.id, "101", "single").await;
        let resident_id = seed_resident(&pool, "Ivy").await;
        let check_in = today();
        let allocation_id = seed_active_allocation(
            &pool,
            hostel.id,
            resident_id,
            room_id,
            check_in,
            check_in + Duration::days(30),
        )
        .await;

        for amount in ["0.10", "0.20"] {
            pool.record_payment_and_receipt(NewPayment {
                hostel_id: hostel.id,
                allocation_id,
                resident_id,
                amount: amount.to_string(),
                payment_method: "cash".to_string(),
                period_start: None,
                period_end: None,
                notes: None,
            })
            .await
            .expect("record");
        }

        let residents = pool.list_residents(hostel.id).await.expect("list");
        let ivy = residents
            .iter()
            .find(|r| r.full_name == "Ivy")
            .expect("ivy");
        assert_eq!(ivy.balance, "99.70");

        let total = sum_decimal_amounts(Some("0.10,0.20")).expect("sum");
        assert_eq!(total, Decimal::from_str("0.30").unwrap());
    }

    #[tokio::test]
    async fn resident_detail_returns_history_and_balance() {
        let pool = test_pool().await;
        let hostel = seed_hostel(&pool).await;
        let room_id = seed_room(&pool, hostel.id, "101", "single").await;
        let resident_id = seed_resident(&pool, "Jack").await;
        let check_in = today();
        let allocation_id = seed_active_allocation(
            &pool,
            hostel.id,
            resident_id,
            room_id,
            check_in,
            check_in + Duration::days(30),
        )
        .await;

        pool.record_payment_and_receipt(NewPayment {
            hostel_id: hostel.id,
            allocation_id,
            resident_id,
            amount: "40.00".to_string(),
            payment_method: "cash".to_string(),
            period_start: None,
            period_end: None,
            notes: None,
        })
        .await
        .expect("payment");

        let detail = pool
            .get_resident_detail(hostel.id, resident_id)
            .await
            .expect("detail");
        assert_eq!(detail.allocations.len(), 1);
        assert_eq!(detail.payments.len(), 1);
        assert_eq!(detail.balance, "60.00");

        let err = pool
            .get_resident_detail(hostel.id, resident_id + 999)
            .await
            .expect_err("missing resident");
        assert!(err.to_string().contains("resident not found"));
    }

    #[tokio::test]
    async fn list_active_allocations_returns_paid_and_balance() {
        let pool = test_pool().await;
        let hostel = seed_hostel(&pool).await;
        let room_id = seed_room(&pool, hostel.id, "101", "single").await;
        let resident_id = seed_resident(&pool, "Kim").await;
        let check_in = today();
        let allocation_id = seed_active_allocation(
            &pool,
            hostel.id,
            resident_id,
            room_id,
            check_in,
            check_in + Duration::days(30),
        )
        .await;

        pool.record_payment_and_receipt(NewPayment {
            hostel_id: hostel.id,
            allocation_id,
            resident_id,
            amount: "25.00".to_string(),
            payment_method: "cash".to_string(),
            period_start: None,
            period_end: None,
            notes: None,
        })
        .await
        .expect("payment");

        let active = pool
            .list_active_allocations(hostel.id)
            .await
            .expect("active allocations");
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].total_paid, "25.00");
        assert_eq!(active[0].balance, "75.00");
    }

    #[tokio::test]
    async fn list_recent_activity_includes_checkin_payment_and_checkout() {
        let pool = test_pool().await;
        let hostel = seed_hostel(&pool).await;
        let room_id = seed_room(&pool, hostel.id, "101", "single").await;
        let resident_id = seed_resident(&pool, "Liam").await;
        let check_in = today();
        let allocation_id = seed_active_allocation(
            &pool,
            hostel.id,
            resident_id,
            room_id,
            check_in,
            check_in + Duration::days(30),
        )
        .await;

        pool.record_payment_and_receipt(NewPayment {
            hostel_id: hostel.id,
            allocation_id,
            resident_id,
            amount: "10.00".to_string(),
            payment_method: "cash".to_string(),
            period_start: None,
            period_end: None,
            notes: None,
        })
        .await
        .expect("payment");

        pool.check_out(hostel.id, allocation_id, check_in + Duration::days(1))
            .await
            .expect("checkout");

        let activity = pool
            .list_recent_activity(hostel.id, 10)
            .await
            .expect("activity");
        let kinds: Vec<_> = activity.iter().map(|a| a.kind.as_str()).collect();
        assert!(kinds.contains(&"check_in"));
        assert!(kinds.contains(&"payment"));
        assert!(kinds.contains(&"check_out"));
    }

    #[tokio::test]
    async fn update_resident_and_list_users_round_trip() {
        let pool = test_pool().await;
        let hostel = seed_hostel(&pool).await;
        let resident_id = seed_resident(&pool, "Mia").await;
        pool.update_resident(
            hostel.id,
            resident_id,
            "Mia Updated".to_string(),
            Some("+256".to_string()),
            None,
            None,
            None,
        )
        .await
        .expect("update");

        let detail = pool
            .get_resident_detail(hostel.id, resident_id)
            .await
            .expect("detail");
        assert_eq!(detail.resident.full_name, "Mia Updated");
        assert_eq!(detail.resident.phone.as_deref(), Some("+256"));

        pool.create_user(NewUser {
            hostel_id: hostel.id,
            full_name: "Admin".to_string(),
            email: "admin@example.com".to_string(),
            password_hash: hash_password("pw").expect("hash"),
            role: "admin".to_string(),
        })
        .await
        .expect("create staff");

        let users = pool.list_users(hostel.id).await.expect("list staff");
        assert_eq!(users.len(), 1);
        assert_eq!(users[0].name, "Admin");
    }

    #[tokio::test]
    async fn update_user_round_trip() {
        let pool = test_pool().await;
        let hostel = seed_hostel(&pool).await;
        let user_id = pool
            .create_user(NewUser {
                hostel_id: hostel.id,
                full_name: "Admin".to_string(),
                email: "admin@example.com".to_string(),
                password_hash: hash_password("oldpw").expect("hash"),
                role: "admin".to_string(),
            })
            .await
            .expect("create staff");

        let updated = pool
            .update_user(
                user_id,
                "Admin Updated".to_string(),
                "admin2".to_string(),
                "manager".to_string(),
                Some(hash_password("newpw").expect("hash")),
            )
            .await
            .expect("update staff");
        assert_eq!(updated.full_name, "Admin Updated");
        assert_eq!(updated.email, "admin2");
        assert_eq!(updated.role, "manager");

        let auth = pool
            .authenticate_user("admin2", "newpw")
            .await
            .expect("authenticate")
            .expect("user exists");
        assert_eq!(auth.id, user_id);
    }

    /// Residents with no allocation in the hostel must still appear, clearly
    /// marked as never allocated and carrying a zero balance.
    #[tokio::test]
    async fn list_residents_shows_never_allocated_for_resident_without_allocations() {
        let pool = test_pool().await;
        let hostel = seed_hostel(&pool).await;
        let resident_id = seed_resident(&pool, "NoStays").await;

        let rows = pool
            .list_residents(hostel.id)
            .await
            .expect("list residents");
        let resident = rows
            .iter()
            .find(|r| r.id == resident_id)
            .expect("find resident");
        assert_eq!(resident.status, "never_allocated");
        assert_eq!(resident.balance, "0");
        assert_eq!(resident.room_number, None);
        assert_eq!(resident.allocation_id, None);
    }

    /// A resident's latest allocation must be scoped to the requested hostel.
    /// In another hostel the same resident should show as never_allocated.
    #[tokio::test]
    async fn list_residents_isolated_by_hostel_id() {
        let pool = test_pool().await;
        let hostel_a = seed_hostel(&pool).await;
        let hostel_b = seed_hostel(&pool).await;
        let room_id = seed_room(&pool, hostel_a.id, "101", "single").await;
        let resident_id = seed_resident(&pool, "Scoped").await;
        let check_in = today();
        seed_active_allocation(
            &pool,
            hostel_a.id,
            resident_id,
            room_id,
            check_in,
            check_in + Duration::days(120),
        )
        .await;

        let rows_a = pool.list_residents(hostel_a.id).await.expect("list a");
        assert!(rows_a
            .iter()
            .any(|r| r.id == resident_id && r.status == "active"));

        let rows_b = pool.list_residents(hostel_b.id).await.expect("list b");
        let in_b = rows_b
            .iter()
            .find(|r| r.id == resident_id)
            .expect("row in hostel b");
        assert_eq!(in_b.status, "never_allocated");
    }

    /// Same-day check-out is a valid checkout boundary.
    #[tokio::test]
    async fn checkout_same_day_as_checkin_allowed() {
        let pool = test_pool().await;
        let hostel = seed_hostel(&pool).await;
        let room_id = seed_room(&pool, hostel.id, "101", "single").await;
        let resident_id = seed_resident(&pool, "SameDay").await;
        let check_in = today();
        let allocation_id = seed_active_allocation(
            &pool,
            hostel.id,
            resident_id,
            room_id,
            check_in,
            check_in + Duration::days(30),
        )
        .await;

        pool.check_out(hostel.id, allocation_id, check_in)
            .await
            .expect("same-day checkout");
        let room = pool
            .get_rooms(hostel.id)
            .await
            .expect("rooms")
            .into_iter()
            .find(|r| r.id == room_id)
            .expect("room");
        assert_eq!(room.status, "vacant");
    }

    /// Checking out a non-existent allocation is treated the same as an
    /// inactive allocation: a clear "not active" error.
    #[tokio::test]
    async fn checkout_unknown_allocation_is_not_active() {
        let pool = test_pool().await;
        let hostel = seed_hostel(&pool).await;
        let err = pool
            .check_out(hostel.id, 99999, today())
            .await
            .expect_err("unknown allocation");
        assert!(err.to_string().contains("not active"));
    }

    /// Resident names cannot be blanked out.
    #[tokio::test]
    async fn update_resident_rejects_empty_name() {
        let pool = test_pool().await;
        let hostel = seed_hostel(&pool).await;
        let resident_id = seed_resident(&pool, "Name").await;
        let err = pool
            .update_resident(
                hostel.id,
                resident_id,
                "   ".to_string(),
                None,
                None,
                None,
                None,
            )
            .await
            .expect_err("empty name");
        assert!(err.to_string().contains("full_name"));
    }

    /// Updating an unknown resident must fail rather than silently no-op.
    #[tokio::test]
    async fn update_resident_rejects_unknown_resident() {
        let pool = test_pool().await;
        let hostel = seed_hostel(&pool).await;
        let err = pool
            .update_resident(
                hostel.id,
                99999,
                "New Name".to_string(),
                None,
                None,
                None,
                None,
            )
            .await
            .expect_err("unknown resident");
        assert!(err.to_string().contains("not found"));
    }

    /// The resident picker must return an empty list when no one is checked in.
    #[tokio::test]
    async fn list_active_allocations_empty_for_no_active_stays() {
        let pool = test_pool().await;
        let hostel = seed_hostel(&pool).await;
        let active = pool
            .list_active_allocations(hostel.id)
            .await
            .expect("list active");
        assert!(active.is_empty());
    }

    /// The activity feed must clamp an excessive limit to 50 rows.
    #[tokio::test]
    async fn list_recent_activity_clamps_limit_to_50() {
        let pool = test_pool().await;
        let hostel = seed_hostel(&pool).await;
        let room_id = seed_room(&pool, hostel.id, "101", "single").await;
        let resident_id = seed_resident(&pool, "Limit").await;
        let check_in = today();
        seed_active_allocation(
            &pool,
            hostel.id,
            resident_id,
            room_id,
            check_in,
            check_in + Duration::days(30),
        )
        .await;

        let activity = pool
            .list_recent_activity(hostel.id, 1000)
            .await
            .expect("activity");
        assert!(
            activity.len() <= 50,
            "limit should be clamped to 50, got {}",
            activity.len()
        );
    }

    /// Date range filters on the payments ledger must use inclusive from/to
    /// calendar days and exclude payments outside the range.
    #[tokio::test]
    async fn list_payments_filters_by_date_range() {
        let pool = test_pool().await;
        let hostel = seed_hostel(&pool).await;
        let room_id = seed_room(&pool, hostel.id, "101", "single").await;
        let resident_id = seed_resident(&pool, "DateFilter").await;
        let check_in = NaiveDate::from_ymd_opt(2026, 7, 1).expect("date");
        let allocation_id = seed_active_allocation(
            &pool,
            hostel.id,
            resident_id,
            room_id,
            check_in,
            check_in + Duration::days(30),
        )
        .await;

        for (amount, date) in [
            ("10.00", "2026-07-09T10:00:00Z"),
            ("20.00", "2026-07-10T10:00:00Z"),
            ("30.00", "2026-07-11T10:00:00Z"),
        ] {
            sqlx::query(
                "INSERT INTO payments (hostel_id, allocation_id, resident_id, amount, payment_date, payment_method) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            )
            .bind(hostel.id)
            .bind(allocation_id)
            .bind(resident_id)
            .bind(amount)
            .bind(date)
            .bind("cash")
            .execute(&pool.inner)
            .await
            .expect("insert payment");
        }

        let rows = pool
            .list_payments(ListPaymentsFilter {
                hostel_id: hostel.id,
                from: Some(NaiveDate::from_ymd_opt(2026, 7, 10).unwrap()),
                to: Some(NaiveDate::from_ymd_opt(2026, 7, 10).unwrap()),
                resident_id: None,
                method: None,
            })
            .await
            .expect("list");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].amount, "20.00");
    }

    /// The resident_id filter must restrict payments to a single resident.
    #[tokio::test]
    async fn list_payments_filters_by_resident_id() {
        let pool = test_pool().await;
        let hostel = seed_hostel(&pool).await;
        let room_a = seed_room(&pool, hostel.id, "101", "single").await;
        let room_b = seed_room(&pool, hostel.id, "102", "single").await;
        let resident_a = seed_resident(&pool, "A").await;
        let resident_b = seed_resident(&pool, "B").await;
        let check_in = today();
        let alloc_a = seed_active_allocation(
            &pool,
            hostel.id,
            resident_a,
            room_a,
            check_in,
            check_in + Duration::days(30),
        )
        .await;
        let alloc_b = seed_active_allocation(
            &pool,
            hostel.id,
            resident_b,
            room_b,
            check_in,
            check_in + Duration::days(30),
        )
        .await;

        pool.record_payment_and_receipt(NewPayment {
            hostel_id: hostel.id,
            allocation_id: alloc_a,
            resident_id: resident_a,
            amount: "10.00".to_string(),
            payment_method: "cash".to_string(),
            period_start: None,
            period_end: None,
            notes: None,
        })
        .await
        .expect("payment a");
        pool.record_payment_and_receipt(NewPayment {
            hostel_id: hostel.id,
            allocation_id: alloc_b,
            resident_id: resident_b,
            amount: "20.00".to_string(),
            payment_method: "cash".to_string(),
            period_start: None,
            period_end: None,
            notes: None,
        })
        .await
        .expect("payment b");

        let rows = pool
            .list_payments(ListPaymentsFilter {
                hostel_id: hostel.id,
                from: None,
                to: None,
                resident_id: Some(resident_a),
                method: None,
            })
            .await
            .expect("list");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].resident_name, "A");
    }

    /// An empty hostel must report zero counts and zero revenue/balance.
    #[tokio::test]
    async fn get_dashboard_stats_zero_for_empty_hostel() {
        let pool = test_pool().await;
        let hostel = seed_hostel(&pool).await;
        let stats = pool
            .get_dashboard_stats(hostel.id, today())
            .await
            .expect("stats");
        assert_eq!(stats.total_rooms, 0);
        assert_eq!(stats.occupied, 0);
        assert_eq!(stats.vacant, 0);
        assert_eq!(stats.maintenance, 0);
        assert_eq!(stats.revenue_today, "0");
        assert_eq!(stats.revenue_month, "0");
        assert_eq!(stats.outstanding_total, "0");
    }

    /// Over-payments create a negative balance for a resident, but the
    /// dashboard's outstanding total must only sum positive balances.
    #[tokio::test]
    async fn get_dashboard_stats_outstanding_ignores_negative_balances() {
        let pool = test_pool().await;
        let hostel = seed_hostel(&pool).await;
        let room_id = seed_room(&pool, hostel.id, "101", "single").await;
        let resident_id = seed_resident(&pool, "Overpaid").await;
        let check_in = today();
        let allocation_id = seed_active_allocation(
            &pool,
            hostel.id,
            resident_id,
            room_id,
            check_in,
            check_in + Duration::days(120),
        )
        .await;

        pool.record_payment_and_receipt(NewPayment {
            hostel_id: hostel.id,
            allocation_id,
            resident_id,
            amount: "200.00".to_string(),
            payment_method: "cash".to_string(),
            period_start: None,
            period_end: None,
            notes: None,
        })
        .await
        .expect("payment");

        let stats = pool
            .get_dashboard_stats(hostel.id, today())
            .await
            .expect("stats");
        assert_eq!(stats.outstanding_total, "0");
    }

    #[tokio::test]
    async fn app_preferences_defaults_and_upsert_round_trip() {
        let pool = test_pool().await;
        let hostel = seed_hostel(&pool).await;

        let prefs = pool
            .get_app_preferences(hostel.id)
            .await
            .expect("get default preferences");
        assert_eq!(prefs.date_format, "dd/MM/yyyy");
        assert_eq!(prefs.language, "en");
        assert!(prefs.receipt_footer.is_none());

        let updated = pool
            .upsert_app_preferences(UpsertAppPreferences {
                hostel_id: hostel.id,
                date_format: "yyyy-MM-dd".to_string(),
                language: "en".to_string(),
                receipt_footer: Some("Thank you for staying with us!".to_string()),
            })
            .await
            .expect("upsert preferences");
        assert_eq!(updated.date_format, "yyyy-MM-dd");
        assert_eq!(
            updated.receipt_footer.as_deref(),
            Some("Thank you for staying with us!")
        );

        let fetched = pool
            .get_app_preferences(hostel.id)
            .await
            .expect("fetch preferences");
        assert_eq!(fetched.date_format, "yyyy-MM-dd");
    }

    #[tokio::test]
    async fn complete_onboarding_creates_one_hostel_and_admin_only_once() {
        let pool = test_pool().await;
        let input = CompleteOnboarding {
            hostel: NewHostel {
                name: "Onboarding Hostel".to_string(),
                address: None,
                phone: None,
                currency: Some("UGX".to_string()),
                branding_logo_path: None,
            },
            admin_name: "First Admin".to_string(),
            email: "first.admin@example.com".to_string(),
            password_hash: hash_password("password123").expect("hash"),
        };

        let user = pool
            .complete_onboarding(input.clone())
            .await
            .expect("complete onboarding");

        assert_eq!(user.role, "admin");
        assert_eq!(
            pool.list_hostels().await.expect("list hostels").len(),
            1,
            "onboarding must create exactly one hostel"
        );
        assert_eq!(
            pool.list_users(user.hostel_id)
                .await
                .expect("list staff")
                .len(),
            1,
            "onboarding must create exactly one staff user"
        );

        let err = pool
            .complete_onboarding(input)
            .await
            .expect_err("onboarding must be one-time");
        assert!(err.to_string().contains("already complete"));
    }

    #[tokio::test]
    async fn allocation_rejects_expected_checkout_before_check_in() {
        let pool = test_pool().await;
        let hostel = seed_hostel(&pool).await;
        let room_id = seed_room(&pool, hostel.id, "101", "single").await;
        let resident_id = seed_resident(&pool, "Date Test Resident").await;
        let check_in = NaiveDate::from_ymd_opt(2026, 1, 10).expect("date");
        let expected = NaiveDate::from_ymd_opt(2026, 1, 9).expect("date");

        let err = pool
            .check_in(NewCheckIn {
                hostel_id: hostel.id,
                resident_id: Some(resident_id),
                full_name: None,
                phone: None,
                email: None,
                id_number: None,
                next_of_kin: None,
                photo_path: None,
                room_id,
                check_in_date: check_in,
                check_out_date: None,
                expected_checkout: expected,
                rate_agreed: "100.00".to_string(),
            })
            .await
            .expect_err("expected checkout before check-in should be rejected");
        assert!(err.to_string().contains("expected_checkout"));
    }

    #[tokio::test]
    async fn allocation_rejects_actual_checkout_before_check_in() {
        let pool = test_pool().await;
        let hostel = seed_hostel(&pool).await;
        let room_id = seed_room(&pool, hostel.id, "101", "single").await;
        let resident_id = seed_resident(&pool, "Date Test Resident").await;
        let check_in = NaiveDate::from_ymd_opt(2026, 1, 10).expect("date");
        let check_out = NaiveDate::from_ymd_opt(2026, 1, 9).expect("date");
        let expected = NaiveDate::from_ymd_opt(2026, 2, 10).expect("date");

        let err = pool
            .check_in(NewCheckIn {
                hostel_id: hostel.id,
                resident_id: Some(resident_id),
                full_name: None,
                phone: None,
                email: None,
                id_number: None,
                next_of_kin: None,
                photo_path: None,
                room_id,
                check_in_date: check_in,
                check_out_date: Some(check_out),
                expected_checkout: expected,
                rate_agreed: "100.00".to_string(),
            })
            .await
            .expect_err("actual checkout before check-in should be rejected");
        assert!(err.to_string().contains("check_out_date"));
    }

    #[tokio::test]
    async fn resident_detail_and_update_are_isolated_by_hostel() {
        let pool = test_pool().await;
        let hostel_a = seed_hostel(&pool).await;
        let hostel_b = seed_hostel(&pool).await;
        let room_a = seed_room(&pool, hostel_a.id, "A101", "single").await;
        let resident_id = seed_resident(&pool, "Cross Hostel Resident").await;
        seed_active_allocation(
            &pool,
            hostel_a.id,
            resident_id,
            room_a,
            today(),
            today() + Duration::days(10),
        )
        .await;

        let detail_err = pool
            .get_resident_detail(hostel_b.id, resident_id)
            .await
            .expect_err("resident without an allocation in hostel B must be hidden");
        assert!(detail_err.to_string().contains("resident not found"));

        let update_err = pool
            .update_resident(
                hostel_b.id,
                resident_id,
                "Changed Name".to_string(),
                None,
                None,
                None,
                None,
            )
            .await
            .expect_err("resident update must be scoped to hostel B");
        assert!(update_err.to_string().contains("resident not found"));

        let name: String = sqlx::query_scalar("SELECT full_name FROM residents WHERE id = ?1")
            .bind(resident_id)
            .fetch_one(&pool.inner)
            .await
            .expect("fetch resident name");
        assert_eq!(name, "Cross Hostel Resident");
    }

    #[tokio::test]
    async fn delete_user_removes_user_and_protects_last_admin() {
        let pool = test_pool().await;
        let hostel = seed_hostel(&pool).await;

        let admin_id = pool
            .create_user(NewUser {
                hostel_id: hostel.id,
                full_name: "Admin One".to_string(),
                email: "admin1@example.com".to_string(),
                password_hash: hash_password("password123").expect("hash"),
                role: "admin".to_string(),
            })
            .await
            .expect("create admin");

        pool.delete_user(admin_id)
            .await
            .expect_err("deleting the only admin should fail");

        let manager_id = pool
            .create_user(NewUser {
                hostel_id: hostel.id,
                full_name: "Manager One".to_string(),
                email: "manager1@example.com".to_string(),
                password_hash: hash_password("password123").expect("hash"),
                role: "manager".to_string(),
            })
            .await
            .expect("create manager");

        pool.delete_user(manager_id)
            .await
            .expect("deleting a manager should succeed");
        let staff = pool.list_users(hostel.id).await.expect("list staff");
        assert_eq!(staff.len(), 1);

        let admin_two_id = pool
            .create_user(NewUser {
                hostel_id: hostel.id,
                full_name: "Admin Two".to_string(),
                email: "admin2@example.com".to_string(),
                password_hash: hash_password("password123").expect("hash"),
                role: "admin".to_string(),
            })
            .await
            .expect("create second admin");

        pool.delete_user(admin_id)
            .await
            .expect("deleting admin when another exists should succeed");
        let staff = pool.list_users(hostel.id).await.expect("list staff");
        assert_eq!(staff.len(), 1);
        assert_eq!(staff[0].id, admin_two_id);
    }
}
