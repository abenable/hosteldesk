use super::support::*;
use db::{AppPreferences, DbPool, UpsertAppPreferences};
use serde::Deserialize;
use tauri::State;
use tracing::{info, instrument};

/// Request payload for [`get_app_preferences`].
#[derive(Debug, Deserialize)]
pub struct GetAppPreferencesRequest {
    pub hostel_id: i64,
}

/// Request payload for [`update_app_preferences`].
#[derive(Debug, Deserialize)]
pub struct UpsertAppPreferencesRequest {
    pub hostel_id: i64,
    pub date_format: String,
    pub language: String,
    pub receipt_footer: Option<String>,
}

/// Read app preferences for a hostel, returning defaults if none exist yet.
#[tauri::command]
#[instrument(skip(state, license_state), fields(command = "get_app_preferences"))]
pub async fn get_app_preferences(
    request: GetAppPreferencesRequest,
    state: State<'_, DbPool>,
    license_state: State<'_, crate::LicenseState>,
    auth_state: State<'_, crate::AuthState>,
) -> Result<AppPreferences, String> {
    info!(hostel_id = request.hostel_id, "get_app_preferences invoked");
    require_license(&license_state)?;
    require_session(&auth_state, request.hostel_id)?;
    state
        .get_app_preferences(request.hostel_id)
        .await
        .map_err(|e| map_db_error("get_app_preferences", e))
}

/// Create or update app preferences for a hostel.
#[tauri::command]
#[instrument(
    skip(state, request, license_state, auth_state),
    fields(command = "update_app_preferences")
)]
pub async fn update_app_preferences(
    request: UpsertAppPreferencesRequest,
    state: State<'_, DbPool>,
    license_state: State<'_, crate::LicenseState>,
    auth_state: State<'_, crate::AuthState>,
) -> Result<AppPreferences, String> {
    info!(
        hostel_id = request.hostel_id,
        "update_app_preferences invoked"
    );
    require_license(&license_state)?;
    require_admin(&auth_state, request.hostel_id)?;

    state
        .upsert_app_preferences(UpsertAppPreferences {
            hostel_id: request.hostel_id,
            date_format: request.date_format,
            language: request.language,
            receipt_footer: request.receipt_footer,
        })
        .await
        .map_err(|e| map_db_error("update_app_preferences", e))
}
