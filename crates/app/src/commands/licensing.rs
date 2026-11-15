use crate::state::AccountLicense;
use tracing::{info, instrument};

/// The account license if this computer may be used today.
#[instrument(skip(state))]
pub fn get_license_status(state: &crate::LicenseState) -> Option<AccountLicense> {
    state.active()
}

/// Read the cached license written by [`store_license`], if any.
pub fn load_license(paths: &crate::paths::AppPaths) -> Option<AccountLicense> {
    let text = std::fs::read_to_string(paths.license_path()).ok()?;
    serde_json::from_str(&text).ok()
}

/// Cache what the server said, so the desk keeps working offline, and
/// update the in-memory state.
pub fn store_license(
    license: AccountLicense,
    state: &crate::LicenseState,
    paths: &crate::paths::AppPaths,
) -> Result<(), String> {
    let path = paths.license_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("could not create data dir: {e}"))?;
    }
    let text = serde_json::to_string_pretty(&license).map_err(|e| e.to_string())?;
    std::fs::write(&path, text).map_err(|e| format!("could not save license: {e}"))?;
    info!(plan = %license.plan, expires_on = %license.expires_on, "account license cached");
    state.set(license);
    Ok(())
}

/// Return the running application version (`CARGO_PKG_VERSION`).
pub fn get_app_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}
