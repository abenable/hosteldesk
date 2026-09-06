use super::support::*;
use db::{DbPool, ResidentDetail, ResidentSummary};
use serde::Deserialize;
use tauri::State;
use tracing::{info, instrument};

/// Request payload for [`list_residents`].
#[derive(Debug, Deserialize)]
pub struct ListResidentsRequest {
    pub hostel_id: i64,
}

/// List every resident in the hostel directory.
#[tauri::command]
#[instrument(skip(state, license_state), fields(command = "list_residents"))]
pub async fn list_residents(
    request: ListResidentsRequest,
    state: State<'_, DbPool>,
    license_state: State<'_, crate::LicenseState>,
    auth_state: State<'_, crate::AuthState>,
) -> Result<Vec<ResidentSummary>, String> {
    info!(hostel_id = request.hostel_id, "list_residents invoked");
    require_license(&license_state)?;
    require_session(&auth_state, request.hostel_id)?;
    state
        .list_residents(request.hostel_id)
        .await
        .map_err(|e| map_db_error("list_residents", e))
}

/// Request payload for [`get_resident_detail`].
#[derive(Debug, Deserialize)]
pub struct GetResidentDetailRequest {
    pub hostel_id: i64,
    pub resident_id: i64,
}

/// Return full resident detail or a clear "resident not found" error.
#[tauri::command]
#[instrument(skip(state, license_state), fields(command = "get_resident_detail"))]
pub async fn get_resident_detail(
    request: GetResidentDetailRequest,
    state: State<'_, DbPool>,
    license_state: State<'_, crate::LicenseState>,
    auth_state: State<'_, crate::AuthState>,
) -> Result<ResidentDetail, String> {
    info!(
        hostel_id = request.hostel_id,
        resident_id = request.resident_id,
        "get_resident_detail invoked"
    );
    require_license(&license_state)?;
    require_session(&auth_state, request.hostel_id)?;
    state
        .get_resident_detail(request.hostel_id, request.resident_id)
        .await
        .map_err(|e| {
            let msg = e.to_string();
            if msg.contains("resident not found") {
                "resident not found".to_string()
            } else {
                map_db_error("get_resident_detail", e)
            }
        })
}

/// Request payload for [`update_resident`].
#[derive(Debug, Deserialize)]
pub struct UpdateResidentRequest {
    pub hostel_id: i64,
    pub resident_id: i64,
    pub full_name: String,
    pub phone: Option<String>,
    pub email: Option<String>,
    pub id_number: Option<String>,
    pub next_of_kin: Option<String>,
    pub next_of_kin_phone: Option<String>,
}

/// Update a resident's contact details.
#[tauri::command]
#[instrument(skip(state, license_state), fields(command = "update_resident"))]
pub async fn update_resident(
    request: UpdateResidentRequest,
    state: State<'_, DbPool>,
    license_state: State<'_, crate::LicenseState>,
    auth_state: State<'_, crate::AuthState>,
) -> Result<(), String> {
    info!(resident_id = request.resident_id, "update_resident invoked");
    require_license(&license_state)?;
    require_session(&auth_state, request.hostel_id)?;
    state
        .update_resident(db::UpdateResident {
            hostel_id: request.hostel_id,
            resident_id: request.resident_id,
            full_name: request.full_name,
            phone: request.phone,
            email: request.email,
            id_number: request.id_number,
            next_of_kin: request.next_of_kin,
            next_of_kin_phone: request.next_of_kin_phone,
        })
        .await
        .map_err(|e| map_db_error("update_resident", e))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::test_support::temp_db_url;

    #[tokio::test]
    async fn list_resident_and_checkout_round_trip() {
        let url = temp_db_url();
        let pool = DbPool::open(&url).await.expect("open database");
        let hostel = pool
            .create_hostel(db::NewHostel {
                name: "App Test Hostel".to_string(),
                address: None,
                phone: None,
                currency: None,
                branding_logo_path: None,
            })
            .await
            .expect("create hostel");
        let room_id = pool
            .add_room(db::NewRoom {
                hostel_id: hostel.id,
                room_number: "101".to_string(),
                room_type: "single".to_string(),
                capacity: 1,
                floor: None,
                semester_rate: "100.00".to_string(),
                recess_rate: "50.00".to_string(),
            })
            .await
            .expect("add room");
        let allocation_id = pool
            .check_in(db::NewCheckIn {
                hostel_id: hostel.id,
                room_id,
                resident_id: None,
                full_name: Some("App Resident".to_string()),
                phone: None,
                email: None,
                id_number: None,
                next_of_kin: None,
                next_of_kin_phone: None,
                photo_path: None,
                rate_agreed: "100.00".to_string(),
                term: "semester_1".to_string(),
                academic_year: "2026/2027".to_string(),
            })
            .await
            .expect("check in");
        let resident_id: i64 =
            sqlx::query_scalar("SELECT resident_id FROM allocations WHERE id = ?1")
                .bind(allocation_id)
                .fetch_one(&pool.inner)
                .await
                .expect("fetch resident id");
        let residents = pool
            .list_residents(hostel.id)
            .await
            .expect("list residents");
        let row = residents
            .iter()
            .find(|r| r.id == resident_id)
            .expect("resident row");
        assert_eq!(row.status, "active");
        assert_eq!(row.room_number.as_deref(), Some("101"));

        pool.check_out(hostel.id, allocation_id)
            .await
            .expect("check out");
        let err = pool
            .check_out(hostel.id, allocation_id)
            .await
            .expect_err("second checkout");
        assert!(err.to_string().contains("not active"));
    }
}
