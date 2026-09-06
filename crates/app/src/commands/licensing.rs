use tauri::AppHandle;
use tauri::State;

use license::LicensePayload;
use tracing::{info, instrument};

/// Return the currently active license payload, if any.
#[tauri::command]
#[instrument(skip(state))]
pub fn get_license_status(state: State<'_, crate::LicenseState>) -> Option<LicensePayload> {
    state.get()
}

/// Validate a license string, persist it, and update in-memory state. Shared
/// by [`activate_license`] and [`activate_license_from_file`].
fn activate_license_string(
    license: &str,
    state: &State<'_, crate::LicenseState>,
    app: &AppHandle,
) -> Result<LicensePayload, String> {
    let payload = license::verify_license(license)
        .map_err(|e| format!("License is invalid or expired: {e}"))?;

    let path = crate::license_path(app)?;
    std::fs::write(&path, license).map_err(|e| format!("could not save license: {e}"))?;
    info!(path = %path.display(), "license saved");

    state.set(payload.clone());
    Ok(payload)
}

/// Open a native "open file" dialog for a license file, then validate and
/// persist it.
#[tauri::command]
#[instrument(skip(state, app))]
pub fn activate_license_from_file(
    state: State<'_, crate::LicenseState>,
    app: AppHandle,
) -> Result<LicensePayload, String> {
    let path = rfd::FileDialog::new()
        .set_title("Select your HostelDesk license file")
        .add_filter("HostelDesk license", &["hdlicense", "lic", "txt"])
        .pick_file()
        .ok_or_else(|| "No file selected".to_string())?;

    let contents =
        std::fs::read_to_string(&path).map_err(|e| format!("could not read license file: {e}"))?;

    activate_license_string(contents.trim(), &state, &app)
}

/// Return the running application version (`CARGO_PKG_VERSION`), shown in
/// Settings so a user has something to quote in a bug report.
#[tauri::command]
pub fn get_app_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}
