use license::LicensePayload;
use tracing::{info, instrument};

/// Return the currently active license payload, if any.
#[instrument(skip(state))]
pub fn get_license_status(state: &crate::LicenseState) -> Option<LicensePayload> {
    state.get()
}

/// Validate a license string (a lease from the server), persist it beside the
/// database so the desk unlocks offline until it expires, and update state.
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

/// Return the running application version (`CARGO_PKG_VERSION`).
pub fn get_app_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}
