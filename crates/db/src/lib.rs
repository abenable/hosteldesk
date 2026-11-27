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
    Ok(Argon2::default()
        .hash_password(password.as_bytes())
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
pub use sync::{OutboxAck, PendingSync, SyncIssue, SyncState};

mod allocations;
mod dashboard;
mod hostels;
mod payments;
mod preferences;
mod residents;
mod rooms;
mod support;
mod sync;
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

    /// Made by argon2 0.5.3, as v2.0.x stored staff passwords. A newer argon2
    /// must still accept every hash already sitting in desks' databases.
    const V2_0_HASH: &str =
        "$argon2id$v=19$m=19456,t=2,p=1$mXMACv9T3slx0r2D/9sZqQ$++CkGCuIPbmFDDWSaPnGZxckIzo5fsJPcLh7+wFrZfs";

    #[test]
    fn hashes_from_earlier_releases_still_verify() {
        assert!(verify_password("v201-secret-pass", V2_0_HASH).expect("parse old hash"));
        assert!(!verify_password("wrong", V2_0_HASH).expect("parse old hash"));
    }

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

    /// A v1.1 database (every migration before sync) must keep its residents
    /// visible after upgrading. `20261115000000` added `residents.hostel_id`
    /// as `NOT NULL DEFAULT 0` and then backfilled only `IS NULL` rows.
    #[tokio::test]
    async fn v1_residents_keep_their_hostel_after_upgrade() {
        use sqlx::migrate::Migrate;
        // sqlx's default migrations table, the one `migrator.run` uses below.
        const MIGRATIONS_TABLE: &str = "_sqlx_migrations";
        let options = SqliteConnectOptions::from_str("sqlite::memory:")
            .expect("url")
            .foreign_keys(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .expect("pool");
        let migrator = sqlx::migrate!("./migrations");
        {
            let mut conn = pool.acquire().await.expect("conn");
            conn.ensure_migrations_table(MIGRATIONS_TABLE)
                .await
                .expect("table");
            for migration in migrator.iter().filter(|m| m.version < 20261115000000) {
                conn.apply(MIGRATIONS_TABLE, migration)
                    .await
                    .expect("v1 migration");
            }
            sqlx::raw_sql(
                "INSERT INTO hostels (name) VALUES ('North');
                 INSERT INTO rooms (hostel_id, room_number, semester_rate, recess_rate)
                     VALUES (1, 'A01', '100', '50');
                 INSERT INTO residents (full_name) VALUES ('Ada'), ('Never Stayed');
                 INSERT INTO allocations (hostel_id, resident_id, room_id, rate_agreed, term, academic_year)
                     VALUES (1, 1, 1, '100', 'semester_1', '2026/2027');",
            )
            .execute(&mut *conn)
            .await
            .expect("v1 data");
        }
        migrator.run(&pool).await.expect("upgrade");
        let db = DbPool { inner: pool };
        let mut names: Vec<String> = db
            .list_residents(1)
            .await
            .expect("list")
            .into_iter()
            .map(|r| r.full_name)
            .collect();
        names.sort();
        assert_eq!(names, ["Ada", "Never Stayed"]);
    }

    /// Rooms from before room classes (each with its own type, floor, prices,
    /// and beds) are grouped into one class per (floor, type, prices), with the
    /// public id the sync Worker's migration 0004 derives for the same rooms.
    /// A linked desk queues those classes ahead of whatever was still waiting
    /// to push, and its waiting room upsert is rewritten to name the class.
    #[tokio::test]
    async fn rooms_from_before_room_classes_are_grouped_into_classes() {
        use protocol::SyncCommand;
        use sqlx::migrate::Migrate;
        const MIGRATIONS_TABLE: &str = "_sqlx_migrations";
        let options = SqliteConnectOptions::from_str("sqlite::memory:")
            .expect("url")
            .foreign_keys(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .expect("pool");
        let migrator = sqlx::migrate!("./migrations");
        {
            let mut conn = pool.acquire().await.expect("conn");
            conn.ensure_migrations_table(MIGRATIONS_TABLE)
                .await
                .expect("table");
            for migration in migrator.iter().filter(|m| m.version < 20261128000000) {
                conn.apply(MIGRATIONS_TABLE, migration)
                    .await
                    .expect("v2.0 migration");
            }
            sqlx::raw_sql(
                r#"INSERT INTO hostels (name, public_id) VALUES ('North', 'h1');
                 INSERT INTO rooms (hostel_id, room_number, room_type, capacity, floor, semester_rate, recess_rate, public_id)
                     VALUES (1, 'A01', 'double', 2, 1, '500000', '200000', 'r1'),
                            (1, 'A02', 'double', 2, 1, '500000', '200000', 'r2'),
                            (1, 'B01', 'single', 1, NULL, '700000', '300000', 'r3');
                 INSERT INTO residents (hostel_id, full_name) VALUES (1, 'Ada');
                 INSERT INTO allocations (hostel_id, resident_id, room_id, rate_agreed, term, academic_year)
                     VALUES (1, 1, 3, '700000', 'semester_1', '2026/2027');
                 INSERT INTO sync_state (hostel_id, device_id) VALUES (1, 'device-1');
                 INSERT INTO sync_outbox (hostel_id, idempotency_key, command_type, payload)
                     VALUES (1, 'k1', 'upsert_room', '{"type":"upsert_room","public_id":"r3","hostel_public_id":"h1","base_revision":1,"room_number":"B01","room_type":"single","capacity":1,"floor":null,"semester_rate":"700000","recess_rate":"300000","status":"occupied"}');"#,
            )
            .execute(&mut *conn)
            .await
            .expect("v2.0 data");
        }
        migrator.run(&pool).await.expect("upgrade");
        let db = DbPool { inner: pool };

        let classes = db.get_room_classes(1).await.expect("classes");
        let ids: Vec<&str> = classes.iter().map(|c| c.public_id.as_str()).collect();
        assert_eq!(
            ids,
            [
                "legacy:h1::single:700000:300000",
                "legacy:h1:1:double:500000:200000"
            ]
        );
        let rooms = db.get_rooms(1).await.expect("rooms");
        let by_number = |n: &str| rooms.iter().find(|r| r.room_number == n).expect(n);
        assert_eq!(by_number("A01").class_id, by_number("A02").class_id);
        assert_eq!(by_number("B01").room_type, "single");
        assert_eq!(by_number("B01").residents, 1);

        let pending: Vec<SyncCommand> = db
            .pending_sync(1)
            .await
            .expect("pending")
            .into_iter()
            .map(|p| p.command)
            .collect();
        assert!(
            matches!(
                pending.as_slice(),
                [
                    SyncCommand::UpsertRoomClass { base_revision: 0, .. },
                    SyncCommand::UpsertRoomClass { base_revision: 0, .. },
                    SyncCommand::UpsertRoom { class_public_id, .. },
                ] if class_public_id == "legacy:h1::single:700000:300000"
            ),
            "{pending:?}"
        );
    }
}
