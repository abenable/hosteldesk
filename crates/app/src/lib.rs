use db::DbPool;
use tauri::{generate_context, generate_handler, AppHandle, Manager};
use tracing::{error, info, instrument};

pub mod commands;
mod state;
pub use state::{AuthState, LicenseState, Session};

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

    // Match the app-id tauri-bundler already uses for packaged Linux builds
    // (deb/AppImage), so a matching .desktop file's icon is picked up by the
    // taskbar/dock under both X11 (WM_CLASS) and Wayland (app_id).
    #[cfg(target_os = "linux")]
    glib::set_prgname(Some("com.hosteldesk.app"));

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

            // Set the window/taskbar icon at runtime. Only needed on Linux:
            // the bundler-embedded icon (tauri.conf.json's `bundle.icon`)
            // already provides the correct window/taskbar/executable icon on
            // Windows and macOS, and running this unconditionally makes
            // every platform's launch depend on this decode succeeding for
            // no benefit on those platforms.
            #[cfg(target_os = "linux")]
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
            commands::licensing::get_app_version,
            commands::rooms::get_rooms,
            commands::allocations::check_in,
            commands::payments::record_payment_and_generate_receipt,
            commands::payments::generate_and_open_receipt,
            commands::licensing::get_license_status,
            commands::licensing::activate_license_from_file,
            commands::users::create_user,
            commands::users::login,
            commands::hostels::complete_onboarding,
            commands::hostels::get_onboarding_status,
            commands::users::get_session,
            commands::users::logout,
            commands::rooms::add_room,
            commands::hostels::get_hostel_profile,
            commands::hostels::list_hostels,
            commands::residents::list_residents,
            commands::residents::get_resident_detail,
            commands::allocations::check_out,
            commands::payments::list_payments,
            commands::allocations::list_active_allocations,
            commands::dashboard::get_dashboard_stats,
            commands::dashboard::list_recent_activity,
            commands::hostels::update_hostel,
            commands::residents::update_resident,
            commands::users::list_users,
            commands::users::update_user,
            commands::users::delete_user,
            commands::backup::backup_database,
            commands::backup::list_backups,
            commands::backup::restore_backup,
            commands::preferences::get_app_preferences,
            commands::preferences::update_app_preferences,
            commands::hostels::upload_hostel_logo,
        ])
        .run(generate_context!());

    if let Err(e) = result {
        error!(error = %e, "error while running tauri application");
        // Release builds run under windows_subsystem = "windows" (no
        // console), so tracing output above is invisible — without this the
        // app just appears to silently fail to start. rfd's message box
        // needs no window/event loop, so it works even though setup failed
        // before any window existed.
        rfd::MessageDialog::new()
            .set_title("HostelDesk failed to start")
            .set_description(format!("{e}"))
            .set_level(rfd::MessageLevel::Error)
            .show();
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
