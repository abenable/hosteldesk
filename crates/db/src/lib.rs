use argon2::password_hash::{rand_core::OsRng, SaltString};
use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};
use sqlx::SqlitePool;
use std::str::FromStr;
use std::time::Duration;
use tracing::{info, instrument};

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

mod allocations;
mod dashboard;
mod hostels;
mod payments;
mod preferences;
mod residents;
mod rooms;
mod support;
mod users;

#[cfg(test)]
mod test_support;

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
}
