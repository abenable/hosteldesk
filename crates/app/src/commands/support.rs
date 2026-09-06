use license::LicensePayload;
use tracing::error;

/// Convert an internal database error into a frontend-friendly string message.
pub(crate) fn map_db_error(command: &str, error: anyhow::Error) -> String {
    error!(command, %error, "command failed");
    format!("Something went wrong while processing your request. {error}")
}

/// Guard commands that require a valid, unexpired license.
pub(crate) fn require_license(
    license_state: &crate::LicenseState,
) -> Result<LicensePayload, String> {
    let payload = license_state
        .get()
        .ok_or_else(|| "unlicensed".to_string())?;
    let today = chrono::Utc::now().date_naive();
    if payload.expiry_date < today {
        return Err("unlicensed".to_string());
    }
    Ok(payload)
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
