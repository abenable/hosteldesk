use license::LicensePayload;
use tracing::{info, instrument};

/// Return the currently active license payload, if any.
#[instrument(skip(state))]
pub fn get_license_status(state: &crate::LicenseState) -> Option<LicensePayload> {
    state.get()
}

/// Validate a license string, persist it, and update in-memory state.
pub fn activate_license_text(
    license: &str,
    state: &crate::LicenseState,
    paths: &crate::paths::AppPaths,
) -> Result<LicensePayload, String> {
    let payload = license::verify_license(license)
        .map_err(|e| format!("License is invalid or expired: {e}"))?;

    let path = paths.license_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("could not create data dir: {e}"))?;
    }
    std::fs::write(&path, license).map_err(|e| format!("could not save license: {e}"))?;
    info!(path = %path.display(), "license saved");

    state.set(payload.clone());
    Ok(payload)
}

/// Open a native file dialog, then validate and persist the chosen license.
pub fn activate_license_from_file(
    state: &crate::LicenseState,
    paths: &crate::paths::AppPaths,
) -> Result<LicensePayload, String> {
    let path = rfd::FileDialog::new()
        .set_title("Select your HostelDesk license file")
        .add_filter("HostelDesk license", &["hdlicense", "lic", "txt"])
        .pick_file()
        .ok_or_else(|| "No file selected".to_string())?;

    let contents =
        std::fs::read_to_string(&path).map_err(|e| format!("could not read license file: {e}"))?;

    activate_license_text(contents.trim(), state, paths)
}

/// Return the running application version (`CARGO_PKG_VERSION`).
pub fn get_app_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}
