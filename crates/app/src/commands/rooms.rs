use super::support::*;
use db::{DbPool, NewRoom, Room};
use hosteldesk_core::currency::parse_positive_money;
use serde::Deserialize;
use tauri::State;
use tracing::{info, instrument};

/// Fetch all rooms for a given hostel.
#[tauri::command(rename_all = "snake_case")]
#[instrument(skip(state, license_state), fields(command = "get_rooms"))]
pub async fn get_rooms(
    hostel_id: i64,
    state: State<'_, DbPool>,
    license_state: State<'_, crate::LicenseState>,
    auth_state: State<'_, crate::AuthState>,
) -> Result<Vec<Room>, String> {
    info!(hostel_id, "get_rooms invoked");
    require_license(&license_state)?;
    require_session(&auth_state, hostel_id)?;
    state
        .get_rooms(hostel_id)
        .await
        .map_err(|e| map_db_error("get_rooms", e))
}

/// Payload for adding a single room.
#[derive(Debug, Deserialize)]
pub struct AddRoomRequest {
    pub hostel_id: i64,
    pub room_number: String,
    pub room_type: String,
    pub capacity: i32,
    pub floor: Option<i32>,
    /// Semester rate as a decimal string, e.g. `"500000.00"`.
    pub semester_rate: String,
    /// Recess/internship rate as a decimal string.
    pub recess_rate: String,
}

/// Add a single room and return the generated room id.
#[tauri::command]
#[instrument(skip(state, license_state), fields(command = "add_room"))]
pub async fn add_room(
    request: AddRoomRequest,
    state: State<'_, DbPool>,
    license_state: State<'_, crate::LicenseState>,
    auth_state: State<'_, crate::AuthState>,
) -> Result<i64, String> {
    info!(
        hostel_id = request.hostel_id,
        room_number = %request.room_number,
        "add_room invoked"
    );
    require_license(&license_state)?;
    require_session(&auth_state, request.hostel_id)?;

    let semester_rate = parse_positive_money(&request.semester_rate).map_err(|e| e.to_string())?;
    let recess_rate = parse_positive_money(&request.recess_rate).map_err(|e| e.to_string())?;
    let room = NewRoom {
        hostel_id: request.hostel_id,
        room_number: request.room_number,
        room_type: request.room_type,
        capacity: request.capacity,
        floor: request.floor,
        semester_rate: semester_rate.to_string(),
        recess_rate: recess_rate.to_string(),
    };
    state
        .add_room(room)
        .await
        .map_err(|e| map_db_error("add_room", e))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::test_support::temp_db_url;

    #[tokio::test]
    async fn get_rooms_returns_empty_for_unknown_hostel() {
        let url = temp_db_url();
        let pool = DbPool::open(&url).await.expect("open database");
        let rooms = pool.get_rooms(9999).await.expect("fetch rooms");
        assert!(rooms.is_empty());
    }
}
