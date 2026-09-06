use super::support::*;
use db::{ActiveAllocation, DbPool, NewCheckIn};
use hosteldesk_core::currency::parse_positive_money;
use serde::Deserialize;
use tauri::State;
use tracing::{error, info, instrument};

/// Payload for an atomic check-in.
///
/// Provide `resident_id` to check in an existing resident, or `full_name` (and
/// optional contact fields) to create a new resident as part of the same
/// transaction.
#[derive(Debug, Deserialize)]
pub struct CheckInRequest {
    pub hostel_id: i64,
    pub room_id: i64,
    pub resident_id: Option<i64>,
    pub full_name: Option<String>,
    pub phone: Option<String>,
    pub email: Option<String>,
    pub id_number: Option<String>,
    pub next_of_kin: Option<String>,
    pub next_of_kin_phone: Option<String>,
    pub photo_path: Option<String>,
    pub rate_agreed: String,
    /// `"semester_1"`, `"semester_2"` or `"recess"`.
    pub term: String,
    /// Academic year label, e.g. `"2026/2027"`.
    pub academic_year: String,
}

/// Atomically check in a new or returning resident.
///
/// `rate_agreed` is validated as a positive monetary amount. Either an
/// existing `resident_id` or `full_name` for a new resident must be supplied.
#[tauri::command]
#[instrument(skip(state, license_state), fields(command = "check_in"))]
pub async fn check_in(
    request: CheckInRequest,
    state: State<'_, DbPool>,
    license_state: State<'_, crate::LicenseState>,
    auth_state: State<'_, crate::AuthState>,
) -> Result<i64, String> {
    info!(
        hostel_id = request.hostel_id,
        room_id = request.room_id,
        "check_in invoked"
    );
    require_license(&license_state)?;
    require_session(&auth_state, request.hostel_id)?;

    let rate_agreed = parse_positive_money(&request.rate_agreed).map_err(|e| e.to_string())?;

    if request.resident_id.is_none() && request.full_name.as_deref().unwrap_or("").trim().is_empty()
    {
        return Err("full_name is required for a new resident".to_string());
    }
    if !matches!(
        request.term.as_str(),
        "semester_1" | "semester_2" | "recess"
    ) {
        return Err("term must be one of: semester_1, semester_2, recess".to_string());
    }
    if !is_academic_year(&request.academic_year) {
        return Err("academic_year must look like \"2026/2027\"".to_string());
    }

    let check_in = NewCheckIn {
        hostel_id: request.hostel_id,
        room_id: request.room_id,
        resident_id: request.resident_id,
        full_name: request.full_name,
        phone: request.phone,
        email: request.email,
        id_number: request.id_number,
        next_of_kin: request.next_of_kin,
        next_of_kin_phone: request.next_of_kin_phone,
        photo_path: request.photo_path,
        rate_agreed: rate_agreed.to_string(),
        term: request.term,
        academic_year: request.academic_year,
    };

    state.check_in(check_in).await.map_err(|e| {
        error!(error = %e, "check_in failed");
        format!("Could not check in resident. {e}")
    })
}

/// Validate an academic year label like `"2026/2027"`: two 4-digit years
/// separated by a slash. Not stricter than that (e.g. year ordering) — it's a
/// free-text label, not a calculated value.
fn is_academic_year(value: &str) -> bool {
    let Some((a, b)) = value.split_once('/') else {
        return false;
    };
    a.len() == 4
        && b.len() == 4
        && a.bytes().all(|c| c.is_ascii_digit())
        && b.bytes().all(|c| c.is_ascii_digit())
}

/// Request payload for [`check_out`].
#[derive(Debug, Deserialize)]
pub struct CheckOutRequest {
    pub hostel_id: i64,
    pub allocation_id: i64,
}

/// Check out an allocation and free its room when no other active stays remain.
#[tauri::command]
#[instrument(skip(state, license_state), fields(command = "check_out"))]
pub async fn check_out(
    request: CheckOutRequest,
    state: State<'_, DbPool>,
    license_state: State<'_, crate::LicenseState>,
    auth_state: State<'_, crate::AuthState>,
) -> Result<(), String> {
    info!(
        hostel_id = request.hostel_id,
        allocation_id = request.allocation_id,
        "check_out invoked"
    );
    require_license(&license_state)?;
    require_session(&auth_state, request.hostel_id)?;
    state
        .check_out(request.hostel_id, request.allocation_id)
        .await
        .map_err(|e| map_db_error("check_out", e))
}

/// Request payload for [`list_active_allocations`].
#[derive(Debug, Deserialize)]
pub struct ListActiveAllocationsRequest {
    pub hostel_id: i64,
}

/// Return active allocations with rolled-up paid/balance for the payment picker.
#[tauri::command]
#[instrument(
    skip(state, license_state),
    fields(command = "list_active_allocations")
)]
pub async fn list_active_allocations(
    request: ListActiveAllocationsRequest,
    state: State<'_, DbPool>,
    license_state: State<'_, crate::LicenseState>,
    auth_state: State<'_, crate::AuthState>,
) -> Result<Vec<ActiveAllocation>, String> {
    info!(
        hostel_id = request.hostel_id,
        "list_active_allocations invoked"
    );
    require_license(&license_state)?;
    require_session(&auth_state, request.hostel_id)?;
    state
        .list_active_allocations(request.hostel_id)
        .await
        .map_err(|e| map_db_error("list_active_allocations", e))
}
