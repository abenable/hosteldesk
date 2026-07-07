use std::sync::Mutex;

use db::DbPool;
use license::LicensePayload;
use tauri::{generate_context, generate_handler, AppHandle, Manager};
use tracing::{error, info, instrument};

pub mod commands;

/// Runtime license state managed by Tauri.
///
/// `None` means the app is locked because no license was supplied or the
/// stored license is invalid/expired.
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

            // Set the window/taskbar icon at runtime (Tauri v2 has no window
            // config field for this; the embedded default is generic).
            if let Some(window) = app.get_webview_window("main") {
                let icon = tauri::image::Image::from_bytes(include_bytes!(
                    "../icons/icon.png"
                ))
                .map_err(|e| format!("invalid icon: {e}"))?;
                window
                    .set_icon(icon)
                    .map_err(|e| format!("could not set window icon: {e}"))?;
            }

            Ok(())
        })
        .invoke_handler(generate_handler![
            commands::get_rooms,
            commands::add_resident,
            commands::create_allocation,
            commands::record_payment,
            commands::record_payment_and_generate_receipt,
            commands::generate_and_open_receipt,
            commands::generate_trial_license,
            commands::get_license_status,
            commands::activate_license,
            commands::create_staff_user,
            commands::login,
            commands::create_hostel,
            commands::add_room,
            commands::bulk_add_rooms,
            commands::get_hostel_profile,
            commands::list_hostels,
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
    use db::DbPool;
    use std::sync::atomic::{AtomicU64, Ordering};

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
}
