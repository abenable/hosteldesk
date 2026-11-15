//! HostelDesk desktop application.
//!
//! The egui shell calls [`Desk`]. The database, license file, and documents
//! folders stay in the locations the Tauri build used (`com.hosteldesk.app`).

use std::sync::OnceLock;

use tracing::error;

pub mod commands;
mod desk;
mod paths;
mod state;
mod sync;
mod ui;

pub use desk::Desk;
pub use paths::AppPaths;
pub use state::{AuthState, LicenseState, Session};

/// Process-wide Tokio runtime. The UI thread drives it with `block_on`.
pub(crate) fn runtime() -> &'static tokio::runtime::Runtime {
    static RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
    RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap_or_else(|e| {
                fail_startup(&format!("could not start the async runtime: {e}"));
            })
    })
}

/// Show a dialog and exit. Used when the process cannot reach the main window.
fn fail_startup(message: &str) -> ! {
    error!(error = %message, "HostelDesk failed to start");
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

/// Open the local database and run the native window.
pub fn run() {
    tracing_subscriber::fmt::init();

    let paths = match AppPaths::resolve() {
        Ok(paths) => paths,
        Err(e) => fail_startup(&e),
    };
    let desk = match runtime().block_on(Desk::open(paths)) {
        Ok(desk) => desk,
        Err(e) => fail_startup(&e),
    };

    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1200.0, 800.0])
            .with_title("HostelDesk")
            .with_app_id(crate::paths::APP_ID),
        ..Default::default()
    };

    if let Err(e) = eframe::run_native(
        "HostelDesk",
        options,
        Box::new(move |_cc| Ok(Box::new(ui::HostelApp::new(desk)))),
    ) {
        fail_startup(&e.to_string());
    }
}
