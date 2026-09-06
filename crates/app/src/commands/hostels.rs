use super::support::*;
use db::{DbPool, Hostel, HostelProfile, NewHostel, User};
use serde::Deserialize;
use tauri::State;
use tauri::{AppHandle, Manager};
use tracing::{info, instrument};

/// Payload for completing first-run onboarding.
#[derive(Debug, Deserialize)]
pub struct CompleteOnboardingRequest {
    pub hostel_name: String,
    pub address: Option<String>,
    pub phone: Option<String>,
    pub currency: Option<String>,
    pub branding_logo_path: Option<String>,
    pub admin_name: String,
    pub email: String,
    pub password: String,
}

/// Return whether one hostel and at least one staff account exist.
#[tauri::command]
#[instrument(skip(state), fields(command = "get_onboarding_status"))]
pub async fn get_onboarding_status(state: State<'_, DbPool>) -> Result<bool, String> {
    info!("get_onboarding_status invoked");
    state
        .is_onboarding_complete()
        .await
        .map_err(|e| map_db_error("get_onboarding_status", e))
}

/// Create the single hostel and its first administrator in one transaction.
///
/// This is the only command that may bootstrap an account without a session.
/// The database operation is one-time, so it cannot be replayed to create a
/// second hostel or another bootstrap administrator.
#[tauri::command]
#[instrument(
    skip(state, request, license_state, auth_state),
    fields(command = "complete_onboarding")
)]
pub async fn complete_onboarding(
    request: CompleteOnboardingRequest,
    state: State<'_, DbPool>,
    license_state: State<'_, crate::LicenseState>,
    auth_state: State<'_, crate::AuthState>,
) -> Result<User, String> {
    info!(hostel_name = %request.hostel_name, "complete_onboarding invoked");
    require_license(&license_state)?;

    if request.hostel_name.trim().is_empty() {
        return Err("hostel name is required".to_string());
    }
    if request.admin_name.trim().is_empty() {
        return Err("admin name is required".to_string());
    }
    if request.email.trim().is_empty() {
        return Err("email is required".to_string());
    }
    if request.password.len() < 8 {
        return Err("password must be at least 8 characters".to_string());
    }

    let password_hash = db::hash_password(&request.password)
        .map_err(|e| format!("Could not hash password: {e}"))?;
    let user = state
        .complete_onboarding(db::CompleteOnboarding {
            hostel: NewHostel {
                name: request.hostel_name,
                address: request.address,
                phone: request.phone,
                currency: request.currency,
                branding_logo_path: request.branding_logo_path,
            },
            admin_name: request.admin_name,
            email: request.email,
            password_hash,
        })
        .await
        .map_err(|e| map_db_error("complete_onboarding", e))?;

    auth_state.set_current_session(user.id, user.hostel_id, user.role.clone());
    Ok(user)
}

/// Return the hostel profile with derived onboarding state, or `None`.
#[tauri::command(rename_all = "snake_case")]
#[instrument(skip(state, license_state), fields(command = "get_hostel_profile"))]
pub async fn get_hostel_profile(
    hostel_id: i64,
    state: State<'_, DbPool>,
    license_state: State<'_, crate::LicenseState>,
    auth_state: State<'_, crate::AuthState>,
) -> Result<Option<HostelProfile>, String> {
    info!(hostel_id, "get_hostel_profile invoked");
    require_license(&license_state)?;
    require_session(&auth_state, hostel_id)?;
    state
        .get_hostel_profile(hostel_id)
        .await
        .map_err(|e| map_db_error("get_hostel_profile", e))
}

/// Return all hostels ordered by id.
#[tauri::command]
#[instrument(skip(state, license_state), fields(command = "list_hostels"))]
pub async fn list_hostels(
    state: State<'_, DbPool>,
    license_state: State<'_, crate::LicenseState>,
    auth_state: State<'_, crate::AuthState>,
) -> Result<Vec<Hostel>, String> {
    info!("list_hostels invoked");
    require_license(&license_state)?;
    let session = require_session_current(&auth_state)?;
    let hostel = state
        .get_hostel_by_id(session.hostel_id)
        .await
        .map_err(|e| map_db_error("list_hostels", e))?
        .ok_or_else(|| "hostel not found".to_string())?;
    Ok(vec![hostel])
}

/// Request payload for [`update_hostel`].
#[derive(Debug, Deserialize)]
pub struct UpdateHostelRequest {
    pub hostel_id: i64,
    pub name: String,
    pub address: Option<String>,
    pub phone: Option<String>,
    pub currency: String,
    pub branding_logo_path: Option<String>,
}

/// Update the hostel profile and return the updated record.
#[tauri::command]
#[instrument(
    skip(state, license_state, auth_state),
    fields(command = "update_hostel")
)]
pub async fn update_hostel(
    request: UpdateHostelRequest,
    state: State<'_, DbPool>,
    license_state: State<'_, crate::LicenseState>,
    auth_state: State<'_, crate::AuthState>,
) -> Result<Hostel, String> {
    info!(hostel_id = request.hostel_id, "update_hostel invoked");
    require_license(&license_state)?;
    require_admin(&auth_state, request.hostel_id)?;
    state
        .update_hostel(
            request.hostel_id,
            request.name,
            request.address,
            request.phone,
            request.currency,
            request.branding_logo_path,
        )
        .await
        .map_err(|e| map_db_error("update_hostel", e))
}

/// Request payload for [`upload_hostel_logo`].
#[derive(Debug, Deserialize)]
pub struct UploadHostelLogoRequest {
    pub hostel_id: i64,
    pub data: Vec<u8>,
    pub filename: String,
}

/// Validate a logo filename and return its lowercased extension.
fn validate_logo_filename(filename: &str) -> Result<String, String> {
    let ext = filename
        .rsplit_once('.')
        .map(|(_, ext)| ext.to_ascii_lowercase())
        .ok_or_else(|| "logo filename must have an extension".to_string())?;

    match ext.as_str() {
        "png" | "jpg" | "jpeg" | "webp" => Ok(ext),
        _ => Err("logo must be a PNG, JPG, or WebP image".to_string()),
    }
}

/// Upload a hostel logo image and persist it under the app data directory.
///
/// The file is saved to `<app-data>/hostels/<hostel_id>/logo.<ext>`. Only PNG,
/// JPG/JPEG and WebP files up to 10 MB are accepted.
#[tauri::command]
#[instrument(
    skip(app, request, license_state, auth_state),
    fields(command = "upload_hostel_logo")
)]
pub async fn upload_hostel_logo(
    request: UploadHostelLogoRequest,
    app: AppHandle,
    license_state: State<'_, crate::LicenseState>,
    auth_state: State<'_, crate::AuthState>,
) -> Result<String, String> {
    info!(hostel_id = request.hostel_id, filename = %request.filename, "upload_hostel_logo invoked");
    require_license(&license_state)?;
    require_admin(&auth_state, request.hostel_id)?;

    const MAX_BYTES: usize = 10 * 1024 * 1024;
    if request.data.len() > MAX_BYTES {
        return Err("logo must be smaller than 10 MB".to_string());
    }

    let ext = validate_logo_filename(&request.filename)?;

    let data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("could not resolve app data dir: {e}"))?;
    let logo_dir = data_dir.join("hostels").join(request.hostel_id.to_string());
    std::fs::create_dir_all(&logo_dir)
        .map_err(|e| format!("could not create logo directory: {e}"))?;

    let logo_path = logo_dir.join(format!("logo.{ext}"));
    std::fs::write(&logo_path, &request.data)
        .map_err(|e| format!("could not write logo file: {e}"))?;

    info!(path = %logo_path.display(), "logo uploaded");
    Ok(logo_path.to_string_lossy().to_string())
}

/// Return the current session without comparing it to a request target.
fn require_session_current(auth_state: &crate::AuthState) -> Result<crate::Session, String> {
    auth_state
        .current_session()
        .ok_or_else(|| "unauthorized".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::test_support::temp_db_url;

    #[tokio::test]
    async fn update_hostel_currency_validation_round_trip() {
        let url = temp_db_url();
        let pool = DbPool::open(&url).await.expect("open database");
        let hostel = pool
            .create_hostel(db::NewHostel {
                name: "Currency Hostel".to_string(),
                address: None,
                phone: None,
                currency: None,
                branding_logo_path: None,
            })
            .await
            .expect("create hostel");

        let err = pool
            .update_hostel(
                hostel.id,
                "Currency Hostel".to_string(),
                None,
                None,
                "ugx".to_string(),
                None,
            )
            .await
            .expect_err("lowercase currency");
        assert!(err.to_string().contains("3-letter uppercase"));

        let updated = pool
            .update_hostel(
                hostel.id,
                "Currency Hostel".to_string(),
                None,
                None,
                "UGX".to_string(),
                None,
            )
            .await
            .expect("valid currency");
        assert_eq!(updated.currency, "UGX");
    }
}
