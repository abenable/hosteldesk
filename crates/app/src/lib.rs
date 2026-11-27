//! HostelDesk desktop application.
//!
//! A Tauri v2 window runs the React frontend in `frontend/`; every IPC command
//! in [`ipc`] forwards to [`Desk`]. The database, license file, and documents
//! folders stay in the `com.hosteldesk.app` data directory ([`AppPaths`]).

use std::sync::Arc;
use std::time::Duration;

use tauri::{AppHandle, Emitter};
use tracing::error;

pub mod commands;
mod desk;
mod ipc;
mod paths;
mod state;
mod sync;

pub use desk::{lock_reason, BootState, Desk, Gate};
pub use paths::AppPaths;
pub use state::{AccountLicense, AuthState, LicenseState, Session, OFFLINE_GRACE_DAYS};
pub use sync::{CloudStatus, ConnectCloudRequest, ConnectResult, SyncEvent, SyncReport};

// Without `custom-protocol` the binary loads devUrl (http://localhost:5173) and
// gives whatever answers there full IPC access, so a release build must have it.
#[cfg(all(not(debug_assertions), not(feature = "custom-protocol")))]
compile_error!("release builds need `--features custom-protocol` (see crates/app/Cargo.toml)");

/// How often the background loop runs a full pass (send, then receive).
const SYNC_EVERY: Duration = Duration::from_secs(30);

/// How often it looks for local changes waiting to send between full passes.
const SEND_CHECK_EVERY: Duration = Duration::from_secs(2);

/// Show a dialog and exit. Used when the process cannot reach the main window.
fn fail_startup(message: &str) -> ! {
    error!(error = %message, "HostelDesk failed to start");
    // Release builds run under windows_subsystem = "windows" (no console),
    // so without this dialog a failed start is silent. rfd needs no window.
    rfd::MessageDialog::new()
        .set_title("HostelDesk failed to start")
        .set_description(message)
        .set_level(rfd::MessageLevel::Error)
        .show();
    std::process::exit(1);
}

/// Replace the current process with a fresh launch of the same binary.
///
/// Restore closes the SQLite pool, so the only safe next step is a new process.
pub(crate) fn relaunch() -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| format!("could not locate HostelDesk: {e}"))?;
    std::process::Command::new(exe)
        .spawn()
        .map_err(|e| format!("could not restart HostelDesk: {e}"))?;
    std::process::exit(0);
}

/// While the desk is enrolled: run [`Desk::sync_now`] every [`SYNC_EVERY`],
/// and sooner (within [`SEND_CHECK_EVERY`]) when local changes are waiting to
/// send, then emit its [`SyncEvent`] to the window as the `sync` event. Staff
/// never have to press "Sync now".
///
/// The early send only follows a pass that worked: offline, or while the
/// account waits for approval, the loop keeps to [`SYNC_EVERY`] rather than
/// calling the server every few seconds. The first pass waits one period: at
/// startup the page has not registered its `sync` listener yet, so the page
/// runs that pass itself (`sync_now`) once it has.
fn spawn_sync_loop(app: AppHandle, desk: Arc<Desk>) {
    tauri::async_runtime::spawn(async move {
        let mut last_pass = tokio::time::Instant::now();
        let mut tick = tokio::time::interval_at(last_pass + SYNC_EVERY, SEND_CHECK_EVERY);
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        let mut working = false;
        loop {
            tick.tick().await;
            if !desk.cloud_enrolled() {
                continue;
            }
            // ponytail: polls a local COUNT every 2 s; wake from the outbox
            // writer instead if that ever shows up in a profile.
            let due = last_pass.elapsed() >= SYNC_EVERY;
            if !due && !(working && desk.has_pending_sync().await) {
                continue;
            }
            let event = desk.sync_now().await;
            last_pass = tokio::time::Instant::now();
            working = event.error.is_none() && event.licensed;
            if let Err(e) = app.emit("sync", &event) {
                error!(error = %e, "could not emit the sync event");
            }
        }
    });
}

/// Open the local database and run the window.
pub fn run() {
    tracing_subscriber::fmt::init();

    // WM_CLASS (X11) and app_id (Wayland) come from the program name; match
    // packaging/linux/com.hosteldesk.app.desktop so the dock shows our icon.
    #[cfg(target_os = "linux")]
    glib::set_prgname(Some(paths::APP_ID));

    let paths = AppPaths::resolve().unwrap_or_else(|e| fail_startup(&e));
    let desk = match tauri::async_runtime::block_on(Desk::open(paths)) {
        Ok(desk) => Arc::new(desk),
        Err(e) => fail_startup(&e),
    };

    let result = tauri::Builder::default()
        .manage(desk.clone())
        .setup(move |app| {
            spawn_sync_loop(app.handle().clone(), desk);
            Ok(())
        })
        .invoke_handler(ipc::handler())
        .run(tauri::generate_context!());

    if let Err(e) = result {
        fail_startup(&e.to_string());
    }
}
