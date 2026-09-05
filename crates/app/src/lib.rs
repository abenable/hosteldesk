use std::sync::Mutex;

use db::DbPool;
use license::LicensePayload;
use tauri::{generate_context, generate_handler, AppHandle, Manager};
use tracing::{error, info, instrument};

pub mod commands;

/// In-memory authorization session for the application's single window.
///
/// This is the backend source of truth for who is currently signed in. The
/// frontend may cache the returned user for display, but it must never be
/// trusted for an access-control decision.
#[derive(Debug, Clone)]
pub struct Session {
    /// Authenticated staff user id.
    pub user_id: i64,
    /// Hostel whose data the user may access.
    pub hostel_id: i64,
    /// Role assigned to the user (`admin` or `manager`).
    pub role: String,
}

/// Current session storage for the single-window desktop app.
#[derive(Debug, Default)]
pub struct AuthState(Mutex<Option<Session>>);

impl AuthState {
    /// Create an empty authorization state.
    pub fn new() -> Self {
        Self::default()
    }

    /// Store or replace the current session.
    pub fn set_current_session(&self, user_id: i64, hostel_id: i64, role: impl Into<String>) {
        if let Ok(mut guard) = self.0.lock() {
            *guard = Some(Session {
                user_id,
                hostel_id,
                role: role.into(),
            });
        } else {
            error!("auth state mutex poisoned");
        }
    }

    /// Return the current session, if any.
    pub fn current_session(&self) -> Option<Session> {
        self.0.lock().ok()?.clone()
    }

    /// Clear the current session.
    pub fn clear_current_session(&self) {
        if let Ok(mut guard) = self.0.lock() {
            *guard = None;
        }
    }
}

/// Runtime license state managed by Tauri.
///
/// `None` means the app is locked because no license was supplied or the
/// stored license is invalid/expired. Data/mutating commands check this state
/// via `commands::require_license` before doing any work.
#[derive(Debug, Default)]
pub struct LicenseState(Mutex<Option<LicensePayload>>);

impl LicenseState {
    /// Replace the current license payload.
    pub fn set(&self, payload: LicensePayload) {
        if let Ok(mut guard) = self.0.lock() {
            *guard = Some(payload);
        } else {
            error!("license state mutex poisoned");
        }
    }

    /// Return the currently active license payload, if any.
    pub fn get(&self) -> Option<LicensePayload> {
        self.0.lock().ok().and_then(|g| g.clone())
    }
}

/// Path to the persisted license file inside the app data directory.
pub(crate) fn license_path(app: &AppHandle) -> Result<std::path::PathBuf, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("could not resolve app data dir: {e}"))?;
    Ok(dir.join("license.lic"))
}

/// Build and run the Tauri application with the SQLite pool as managed state.
pub fn run() {
    tracing_subscriber::fmt::init();

    let result = tauri::Builder::default()
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&data_dir)?;

            let db_path = data_dir.join("hosteldesk.db");
            let db_url = format!("sqlite:{}", db_path.display());
            let pool = tauri::async_runtime::block_on(DbPool::open(&db_url))?;

            app.manage(pool);
            info!(db_path = %db_path.display(), "database pool initialized");

            let license_state = load_license_state(app.handle())?;
            app.manage(license_state);

            app.manage(AuthState::new());
            info!("auth state initialized");

            // Set the window/taskbar icon at runtime (Tauri v2 has no window
            // config field for this; the embedded default is generic).
            if let Some(window) = app.get_webview_window("main") {
                let icon = tauri::image::Image::from_bytes(include_bytes!("../icons/icon.png"))
                    .map_err(|e| format!("invalid icon: {e}"))?;
                window
                    .set_icon(icon)
                    .map_err(|e| format!("could not set window icon: {e}"))?;
            }

            Ok(())
        })
        .invoke_handler(generate_handler![
            commands::get_rooms,
            commands::check_in,
            commands::record_payment_and_generate_receipt,
            commands::generate_and_open_receipt,
            commands::get_license_status,
            commands::activate_license,
            commands::create_staff_user,
            commands::login,
            commands::complete_onboarding,
            commands::get_onboarding_status,
            commands::get_session,
            commands::logout,
            commands::add_room,
            commands::get_hostel_profile,
            commands::list_hostels,
            commands::list_residents,
            commands::get_resident_detail,
            commands::check_out,
            commands::list_payments,
            commands::list_active_allocations,
            commands::get_dashboard_stats,
            commands::list_recent_activity,
            commands::update_hostel,
            commands::update_resident,
            commands::list_staff_users,
            commands::update_staff_user,
            commands::delete_staff_user,
            commands::backup_database,
            commands::list_backups,
            commands::restore_backup,
            commands::get_app_preferences,
            commands::update_app_preferences,
            commands::upload_hostel_logo,
        ])
        .run(generate_context!());

    if let Err(e) = result {
        error!(error = %e, "error while running tauri application");
        std::process::exit(1);
    }
}

/// Load any persisted license and verify it.
#[instrument(skip(app))]
fn load_license_state(app: &AppHandle) -> Result<LicenseState, String> {
    let state = LicenseState::default();
    let path = license_path(app)?;

    if !path.exists() {
        info!("no persisted license file found");
        return Ok(state);
    }

    let text =
        std::fs::read_to_string(&path).map_err(|e| format!("could not read license file: {e}"))?;

    match license::verify_license(&text) {
        Ok(payload) => {
            info!(hostel_name = %payload.hostel_name, tier = %payload.tier, "license verified");
            state.set(payload);
        }
        Err(e) => {
            error!(error = %e, path = %path.display(), "stored license is invalid or expired");
        }
    }

    Ok(state)
}

#[cfg(test)]
mod tests {
    use crate::{commands, AuthState, LicenseState};
    use db::DbPool;
    use std::sync::atomic::{AtomicU64, Ordering};
    use tauri::Manager;

    static COUNTER: AtomicU64 = AtomicU64::new(0);

    fn temp_db_url() -> String {
        let index = COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "hosteldesk_app_test_{}_{}.db",
            std::process::id(),
            index
        ));
        format!("sqlite:{}", path.display())
    }

    #[tokio::test]
    async fn get_rooms_returns_empty_for_unknown_hostel() {
        let url = temp_db_url();
        let pool = DbPool::open(&url).await.expect("open database");
        let rooms = pool.get_rooms(9999).await.expect("fetch rooms");
        assert!(rooms.is_empty());
    }

    #[test]
    fn password_hash_and_verify_round_trip() {
        let password = "a_secure_password_123";
        let hash = db::hash_password(password).expect("hashing should succeed");
        assert!(db::verify_password(password, &hash).expect("verification should not fail"));
        assert!(
            !db::verify_password("wrong_password", &hash).expect("verification should not fail")
        );
    }

    #[tokio::test]
    async fn create_and_login_staff_user_round_trip() {
        let url = temp_db_url();
        let pool = DbPool::open(&url).await.expect("open database");

        // A staff user requires an existing hostel because of the foreign key.
        sqlx::query("INSERT INTO hostels (name, currency) VALUES (?1, ?2)")
            .bind("Test Hostel")
            .bind("USD")
            .execute(&pool.inner)
            .await
            .expect("insert hostel");
        let hostel_id = sqlx::query_scalar::<_, i64>("SELECT id FROM hostels WHERE name = ?1")
            .bind("Test Hostel")
            .fetch_one(&pool.inner)
            .await
            .expect("fetch hostel id");

        let password = "frontdesk_2024";
        let id = pool
            .create_staff_user(db::NewStaffUser {
                hostel_id,
                name: "Test User".to_string(),
                username: "testuser".to_string(),
                password_hash: db::hash_password(password).expect("hash"),
                role: "manager".to_string(),
            })
            .await
            .expect("create staff user");
        assert!(id > 0);

        let user = pool
            .authenticate_staff_user("testuser", password)
            .await
            .expect("authenticate")
            .expect("user should exist");
        assert_eq!(user.username, "testuser");
        assert_eq!(user.role, "manager");

        let none = pool
            .authenticate_staff_user("testuser", "wrong_password")
            .await
            .expect("authenticate");
        assert!(none.is_none());
    }

    #[tokio::test]
    async fn list_resident_and_checkout_round_trip() {
        let url = temp_db_url();
        let pool = DbPool::open(&url).await.expect("open database");
        let hostel = pool
            .create_hostel(db::NewHostel {
                name: "App Test Hostel".to_string(),
                address: None,
                phone: None,
                currency: None,
                branding_logo_path: None,
            })
            .await
            .expect("create hostel");
        let room_id = pool
            .add_room(db::NewRoom {
                hostel_id: hostel.id,
                room_number: "101".to_string(),
                room_type: "single".to_string(),
                capacity: 1,
                floor: None,
                rate_per_period: "100.00".to_string(),
                billing_period: None,
            })
            .await
            .expect("add room");
        let check_in = chrono::Utc::now().date_naive();
        let allocation_id = pool
            .check_in(db::NewCheckIn {
                hostel_id: hostel.id,
                room_id,
                resident_id: None,
                full_name: Some("App Resident".to_string()),
                phone: None,
                email: None,
                id_number: None,
                next_of_kin: None,
                photo_path: None,
                check_in_date: check_in,
                check_out_date: None,
                expected_checkout: check_in + chrono::Duration::days(120),
                rate_agreed: "100.00".to_string(),
            })
            .await
            .expect("check in");
        let resident_id: i64 =
            sqlx::query_scalar("SELECT resident_id FROM allocations WHERE id = ?1")
                .bind(allocation_id)
                .fetch_one(&pool.inner)
                .await
                .expect("fetch resident id");
        let residents = pool
            .list_residents(hostel.id)
            .await
            .expect("list residents");
        let row = residents
            .iter()
            .find(|r| r.id == resident_id)
            .expect("resident row");
        assert_eq!(row.status, "active");
        assert_eq!(row.room_number.as_deref(), Some("101"));

        pool.check_out(
            hostel.id,
            allocation_id,
            check_in + chrono::Duration::days(1),
        )
        .await
        .expect("check out");
        let err = pool
            .check_out(
                hostel.id,
                allocation_id,
                check_in + chrono::Duration::days(2),
            )
            .await
            .expect_err("second checkout");
        assert!(err.to_string().contains("not active"));
    }

    #[tokio::test]
    async fn backup_database_creates_valid_copy() {
        let url = temp_db_url();
        let pool = DbPool::open(&url).await.expect("open database");

        pool.create_hostel(db::NewHostel {
            name: "Backup Test Hostel".to_string(),
            address: None,
            phone: None,
            currency: None,
            branding_logo_path: None,
        })
        .await
        .expect("create hostel");

        let backup_path = std::env::temp_dir().join(format!(
            "hosteldesk_backup_test_{}_{}.db",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        let path_str = backup_path.to_string_lossy().to_string();

        sqlx::query("VACUUM INTO ?1")
            .bind(&path_str)
            .execute(&pool.inner)
            .await
            .expect("vacuum into");

        assert!(backup_path.exists(), "backup file should exist");

        let backup_url = format!("sqlite:{}", backup_path.display());
        let backup_pool = sqlx::SqlitePool::connect(&backup_url)
            .await
            .expect("open backup");
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM hostels")
            .fetch_one(&backup_pool)
            .await
            .expect("count hostels");
        assert_eq!(count, 1, "backup should contain the hostel row");

        drop(backup_pool);
        let _ = std::fs::remove_file(&backup_path);
    }

    #[tokio::test]
    async fn restore_backup_does_not_replay_stale_wal() {
        let live_temp = tempfile::tempdir().expect("create live temp dir");
        let live_path = live_temp.path().join("hosteldesk.db");
        let live_url = format!("sqlite:{}", live_path.display());

        let backup_temp = tempfile::tempdir().expect("create backup temp dir");
        let backup_path = backup_temp.path().join("hosteldesk.db");
        let backup_url = format!("sqlite:{}", backup_path.display());

        // Create the backup database with a known row and flush the WAL so the
        // base file is self-contained — copying the base file alone is enough.
        {
            let pool = DbPool::open(&backup_url).await.expect("open backup");
            pool.create_hostel(db::NewHostel {
                name: "Backup Hostel".to_string(),
                address: None,
                phone: None,
                currency: None,
                branding_logo_path: None,
            })
            .await
            .expect("create backup hostel");
            let mut conn = pool.inner.acquire().await.expect("acquire connection");
            let _ = sqlx::query("PRAGMA wal_checkpoint(TRUNCATE)")
                .execute(&mut *conn)
                .await;
            drop(conn);
            pool.inner.close().await;
        }

        // Create the live database with a different row. Disable auto-checkpoint
        // so the commit stays in the WAL, simulating the pre-restore state that
        // the restore fix must clear.
        {
            let pool = DbPool::open(&live_url).await.expect("open live");
            let mut conn = pool.inner.acquire().await.expect("acquire connection");
            let _ = sqlx::query("PRAGMA wal_autocheckpoint=0")
                .execute(&mut *conn)
                .await;
            let _ = sqlx::query("INSERT INTO hostels (name, currency) VALUES (?1, ?2)")
                .bind("Live Hostel")
                .bind("USD")
                .execute(&mut *conn)
                .await
                .expect("insert live hostel");
            drop(conn);
            pool.inner.close().await;
        }

        // Copy the backup over the live base file, then delete WAL/SHM sidecars
        // exactly as restore_backup does.
        std::fs::copy(&backup_path, &live_path).expect("copy backup over live");
        let _ = std::fs::remove_file(format!("{}-wal", live_path.display()));
        let _ = std::fs::remove_file(format!("{}-shm", live_path.display()));

        // No stale sidecars should remain before the next open.
        let stale_wal_exists =
            std::path::PathBuf::from(format!("{}-wal", live_path.display())).exists();
        let stale_shm_exists =
            std::path::PathBuf::from(format!("{}-shm", live_path.display())).exists();
        assert!(
            !stale_wal_exists && !stale_shm_exists,
            "restore should remove stale WAL/SHM sidecars"
        );

        // Reopening should see only the backup row; any stale WAL must not replay.
        let restored = DbPool::open(&live_url).await.expect("reopen restored");
        let names: Vec<String> = sqlx::query_scalar("SELECT name FROM hostels ORDER BY name")
            .fetch_all(&restored.inner)
            .await
            .expect("fetch hostels");
        assert_eq!(
            names,
            vec!["Backup Hostel"],
            "restored database should reflect backup, not stale WAL"
        );
    }

    #[tokio::test]
    async fn update_hostel_currency_validation_round_trip() {
        let url = temp_db_url();
        let pool = DbPool::open(&url).await.expect("open database");
        let hostel = pool
            .create_hostel(db::NewHostel {
                name: "Currency Hostel".to_string(),
                address: None,
                phone: None,
                currency: None,
                branding_logo_path: None,
            })
            .await
            .expect("create hostel");

        let err = pool
            .update_hostel(
                hostel.id,
                "Currency Hostel".to_string(),
                None,
                None,
                "ugx".to_string(),
                None,
            )
            .await
            .expect_err("lowercase currency");
        assert!(err.to_string().contains("3-letter uppercase"));

        let updated = pool
            .update_hostel(
                hostel.id,
                "Currency Hostel".to_string(),
                None,
                None,
                "UGX".to_string(),
                None,
            )
            .await
            .expect("valid currency");
        assert_eq!(updated.currency, "UGX");
    }

    async fn setup_payment_fixture(pool: &DbPool) -> (i64, i64, i64, i64) {
        let hostel = pool
            .create_hostel(db::NewHostel {
                name: "Money Test Hostel".to_string(),
                address: None,
                phone: None,
                currency: None,
                branding_logo_path: None,
            })
            .await
            .expect("create hostel");
        let room_id = pool
            .add_room(db::NewRoom {
                hostel_id: hostel.id,
                room_number: "101".to_string(),
                room_type: "single".to_string(),
                capacity: 1,
                floor: None,
                rate_per_period: "100.00".to_string(),
                billing_period: None,
            })
            .await
            .expect("add room");
        let check_in = chrono::Utc::now().date_naive();
        let allocation_id = pool
            .check_in(db::NewCheckIn {
                hostel_id: hostel.id,
                room_id,
                resident_id: None,
                full_name: Some("Money Test Resident".to_string()),
                phone: None,
                email: None,
                id_number: None,
                next_of_kin: None,
                photo_path: None,
                check_in_date: check_in,
                check_out_date: None,
                expected_checkout: check_in + chrono::Duration::days(7),
                rate_agreed: "100.00".to_string(),
            })
            .await
            .expect("check in");
        let resident_id: i64 =
            sqlx::query_scalar("SELECT resident_id FROM allocations WHERE id = ?1")
                .bind(allocation_id)
                .fetch_one(&pool.inner)
                .await
                .expect("fetch resident id");
        (hostel.id, room_id, resident_id, allocation_id)
    }

    fn setup_license_state() -> LicenseState {
        let state = LicenseState::default();
        state.set(license::LicensePayload {
            hostel_name: "Money Test Hostel".to_string(),
            expiry_date: chrono::Utc::now().date_naive() + chrono::Duration::days(30),
            tier: "trial".to_string(),
        });
        state
    }

    #[tokio::test]
    async fn record_payment_and_generate_receipt_rejects_negative_amount() {
        let url = temp_db_url();
        let pool = DbPool::open(&url).await.expect("open database");
        let (hostel_id, _room_id, resident_id, allocation_id) = setup_payment_fixture(&pool).await;
        let license_state = setup_license_state();

        let app = tauri::test::mock_app();
        app.manage(pool);
        app.manage(license_state);
        app.manage(AuthState::new());

        let request = commands::RecordPaymentRequest {
            hostel_id,
            allocation_id,
            resident_id,
            amount: "-1".to_string(),
            payment_method: "cash".to_string(),
            period_start: None,
            period_end: None,
            notes: None,
        };

        let result = commands::record_payment_and_generate_receipt(
            request,
            app.state::<DbPool>(),
            app.state::<LicenseState>(),
            app.state::<AuthState>(),
        )
        .await;
        assert!(result.is_err(), "negative amount should be rejected");

        let payment_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM payments")
            .fetch_one(&app.state::<DbPool>().inner)
            .await
            .expect("count payments");
        assert_eq!(payment_count, 0, "no payment row should be inserted");

        let receipt_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM receipts")
            .fetch_one(&app.state::<DbPool>().inner)
            .await
            .expect("count receipts");
        assert_eq!(receipt_count, 0, "no receipt row should be inserted");
    }

    #[tokio::test]
    async fn record_payment_and_generate_receipt_rejects_zero_amount() {
        let url = temp_db_url();
        let pool = DbPool::open(&url).await.expect("open database");
        let (hostel_id, _room_id, resident_id, allocation_id) = setup_payment_fixture(&pool).await;
        let license_state = setup_license_state();

        let app = tauri::test::mock_app();
        app.manage(pool);
        app.manage(license_state);
        app.manage(AuthState::new());

        let request = commands::RecordPaymentRequest {
            hostel_id,
            allocation_id,
            resident_id,
            amount: "0".to_string(),
            payment_method: "cash".to_string(),
            period_start: None,
            period_end: None,
            notes: None,
        };

        let result = commands::record_payment_and_generate_receipt(
            request,
            app.state::<DbPool>(),
            app.state::<LicenseState>(),
            app.state::<AuthState>(),
        )
        .await;
        assert!(result.is_err(), "zero amount should be rejected");

        let payment_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM payments")
            .fetch_one(&app.state::<DbPool>().inner)
            .await
            .expect("count payments");
        assert_eq!(payment_count, 0, "no payment row should be inserted");

        let receipt_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM receipts")
            .fetch_one(&app.state::<DbPool>().inner)
            .await
            .expect("count receipts");
        assert_eq!(receipt_count, 0, "no receipt row should be inserted");
    }

    #[tokio::test]
    async fn payments_check_constraint_rejects_negative_amount() {
        let url = temp_db_url();
        let pool = DbPool::open(&url).await.expect("open database");
        let (hostel_id, _room_id, resident_id, allocation_id) = setup_payment_fixture(&pool).await;

        let err = sqlx::query(
            "INSERT INTO payments (hostel_id, allocation_id, resident_id, amount, payment_method)
             VALUES (?1, ?2, ?3, ?4, ?5)",
        )
        .bind(hostel_id)
        .bind(allocation_id)
        .bind(resident_id)
        .bind("-1.00")
        .bind("cash")
        .execute(&pool.inner)
        .await
        .expect_err("negative amount should violate the CHECK constraint");
        assert!(
            err.to_string().contains("CHECK") || err.to_string().contains("constraint"),
            "error should be a CHECK constraint failure: {err}"
        );
    }

    #[tokio::test]
    async fn payments_check_constraint_rejects_zero_amount() {
        let url = temp_db_url();
        let pool = DbPool::open(&url).await.expect("open database");
        let (hostel_id, _room_id, resident_id, allocation_id) = setup_payment_fixture(&pool).await;

        let err = sqlx::query(
            "INSERT INTO payments (hostel_id, allocation_id, resident_id, amount, payment_method)
             VALUES (?1, ?2, ?3, ?4, ?5)",
        )
        .bind(hostel_id)
        .bind(allocation_id)
        .bind(resident_id)
        .bind("0.00")
        .bind("cash")
        .execute(&pool.inner)
        .await
        .expect_err("zero amount should violate the CHECK constraint");
        assert!(
            err.to_string().contains("CHECK") || err.to_string().contains("constraint"),
            "error should be a CHECK constraint failure: {err}"
        );
    }

    async fn setup_staff_user(
        pool: &DbPool,
        name: &str,
        username: &str,
        role: &str,
    ) -> (i64, String) {
        let password = format!("password_{username}");
        let id = pool
            .create_staff_user(db::NewStaffUser {
                hostel_id: 1,
                name: name.to_string(),
                username: username.to_string(),
                password_hash: db::hash_password(&password).expect("hash"),
                role: role.to_string(),
            })
            .await
            .expect("create staff user");
        (id, password)
    }

    #[tokio::test]
    async fn manager_session_cannot_delete_staff_user() {
        let url = temp_db_url();
        let pool = DbPool::open(&url).await.expect("open database");

        sqlx::query("INSERT INTO hostels (name, currency) VALUES (?1, ?2)")
            .bind("Auth Test Hostel")
            .bind("USD")
            .execute(&pool.inner)
            .await
            .expect("insert hostel");

        let (admin_id, _admin_password) = setup_staff_user(&pool, "Admin", "admin", "admin").await;
        let (manager_id, _manager_password) =
            setup_staff_user(&pool, "Manager", "manager", "manager").await;

        let license_state = setup_license_state();
        let auth_state = AuthState::new();
        auth_state.set_current_session(manager_id, 1, "manager");

        let app = tauri::test::mock_app();
        app.manage(pool);
        app.manage(license_state);
        app.manage(AuthState::new());
        app.manage(auth_state);

        let result = commands::delete_staff_user(
            commands::DeleteStaffUserRequest { user_id: admin_id },
            app.state::<DbPool>(),
            app.state::<LicenseState>(),
            app.state::<AuthState>(),
        )
        .await;

        let err = result.expect_err("manager should not be able to delete staff users");
        assert!(
            err.contains("forbidden") || err.contains("unauthorized"),
            "expected authorization error, got: {err}"
        );

        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM staff_users WHERE id = ?1")
            .bind(admin_id)
            .fetch_one(&app.state::<DbPool>().inner)
            .await
            .expect("count admin user");
        assert_eq!(count, 1, "admin should not have been deleted");
    }

    #[tokio::test]
    async fn manager_session_cannot_escalate_own_role() {
        let url = temp_db_url();
        let pool = DbPool::open(&url).await.expect("open database");

        sqlx::query("INSERT INTO hostels (name, currency) VALUES (?1, ?2)")
            .bind("Auth Test Hostel")
            .bind("USD")
            .execute(&pool.inner)
            .await
            .expect("insert hostel");

        let (admin_id, _admin_password) = setup_staff_user(&pool, "Admin", "admin", "admin").await;
        let (manager_id, _manager_password) =
            setup_staff_user(&pool, "Manager", "manager", "manager").await;

        let license_state = setup_license_state();
        let auth_state = AuthState::new();
        auth_state.set_current_session(manager_id, 1, "manager");

        let app = tauri::test::mock_app();
        app.manage(pool);
        app.manage(license_state);
        app.manage(AuthState::new());
        app.manage(auth_state);

        let result = commands::update_staff_user(
            commands::UpdateStaffUserRequest {
                user_id: manager_id,
                name: "Manager".to_string(),
                username: "manager".to_string(),
                role: "admin".to_string(),
                password: None,
            },
            app.state::<DbPool>(),
            app.state::<LicenseState>(),
            app.state::<AuthState>(),
        )
        .await;

        let err = result.expect_err("manager should not be able to escalate their role");
        assert!(
            err.contains("forbidden") || err.contains("unauthorized"),
            "expected authorization error, got: {err}"
        );

        let role: String = sqlx::query_scalar("SELECT role FROM staff_users WHERE id = ?1")
            .bind(manager_id)
            .fetch_one(&app.state::<DbPool>().inner)
            .await
            .expect("fetch manager role");
        assert_eq!(role, "manager", "manager role should remain unchanged");

        // Ensure the change did not inadvertently land on the admin.
        let admin_role: String = sqlx::query_scalar("SELECT role FROM staff_users WHERE id = ?1")
            .bind(admin_id)
            .fetch_one(&app.state::<DbPool>().inner)
            .await
            .expect("fetch admin role");
        assert_eq!(admin_role, "admin");
    }
}
