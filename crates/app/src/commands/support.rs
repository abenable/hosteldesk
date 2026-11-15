use crate::state::AccountLicense;
use tracing::error;

/// Convert an internal database error into a frontend-friendly string message.
pub(crate) fn map_db_error(command: &str, error: anyhow::Error) -> String {
    error!(command, %error, "command failed");
    format!("Something went wrong while processing your request. {error}")
}

/// Guard commands that require the account's license to be usable today.
pub(crate) fn require_license(
    license_state: &crate::LicenseState,
) -> Result<AccountLicense, String> {
    license_state
        .active()
        .ok_or_else(|| "unlicensed".to_string())
}

/// Guard commands that require any authenticated session for the target hostel.
pub(crate) fn require_session(
    auth_state: &crate::AuthState,
    hostel_id: i64,
) -> Result<crate::Session, String> {
    let session = auth_state
        .current_session()
        .ok_or_else(|| "unauthorized".to_string())?;
    if session.hostel_id != hostel_id {
        return Err("forbidden".to_string());
    }
    Ok(session)
}

/// Guard admin-only commands for the target hostel.
pub(crate) fn require_admin(
    auth_state: &crate::AuthState,
    hostel_id: i64,
) -> Result<crate::Session, String> {
    let session = require_session(auth_state, hostel_id)?;
    if session.role != "admin" {
        return Err("forbidden".to_string());
    }
    Ok(session)
}
