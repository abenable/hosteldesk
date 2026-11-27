use super::support::*;
use db::{DbPool, NewRoom, NewRoomClass, Room, RoomClass};
use hosteldesk_core::currency::parse_positive_money;
use serde::Deserialize;
use tracing::{info, instrument};

/// Fetch all rooms for a given hostel.
#[instrument(skip(state, license_state), fields(command = "get_rooms"))]
pub async fn get_rooms(
    hostel_id: i64,
    state: &DbPool,
    license_state: &crate::LicenseState,
    auth_state: &crate::AuthState,
) -> Result<Vec<Room>, String> {
    info!(hostel_id, "get_rooms invoked");
    require_license(license_state)?;
    require_session(auth_state, hostel_id)?;
    state
        .get_rooms(hostel_id)
        .await
        .map_err(|e| map_db_error("get_rooms", e))
}

/// Room classes for a given hostel.
#[instrument(skip(state, license_state), fields(command = "get_room_classes"))]
pub async fn get_room_classes(
    hostel_id: i64,
    state: &DbPool,
    license_state: &crate::LicenseState,
    auth_state: &crate::AuthState,
) -> Result<Vec<RoomClass>, String> {
    require_license(license_state)?;
    require_session(auth_state, hostel_id)?;
    state
        .get_room_classes(hostel_id)
        .await
        .map_err(|e| map_db_error("get_room_classes", e))
}

/// Payload for creating a room class, or editing one when `class_id` is set.
#[derive(Debug, Deserialize)]
pub struct SaveRoomClassRequest {
    pub hostel_id: i64,
    /// The class to edit; `None` creates one.
    pub class_id: Option<i64>,
    pub floor: Option<i32>,
    /// `single`, `double`, or `dorm`.
    pub room_type: String,
    /// Semester rate as a decimal string, e.g. `"500000.00"`.
    pub semester_rate: String,
    /// Recess/internship rate as a decimal string.
    pub recess_rate: String,
}

/// Create or edit a room class and return its id.
#[instrument(skip(state, license_state), fields(command = "save_room_class"))]
pub async fn save_room_class(
    request: SaveRoomClassRequest,
    state: &DbPool,
    license_state: &crate::LicenseState,
    auth_state: &crate::AuthState,
) -> Result<i64, String> {
    require_license(license_state)?;
    // Classes and their prices are hostel setup, which only an administrator changes.
    require_admin(auth_state, request.hostel_id)?;
    let semester_rate = parse_positive_money(&request.semester_rate).map_err(|e| e.to_string())?;
    let recess_rate = parse_positive_money(&request.recess_rate).map_err(|e| e.to_string())?;
    let class = NewRoomClass {
        hostel_id: request.hostel_id,
        floor: request.floor,
        room_type: request.room_type,
        semester_rate: semester_rate.to_string(),
        recess_rate: recess_rate.to_string(),
    };
    match request.class_id {
        Some(class_id) => state
            .update_room_class(class_id, class)
            .await
            .map(|()| class_id),
        None => state.add_room_class(class).await,
    }
    .map_err(|e| map_db_error("save_room_class", e))
}

/// Payload for adding rooms of one class.
#[derive(Debug, Deserialize)]
pub struct AddRoomsRequest {
    pub hostel_id: i64,
    pub class_id: i64,
    /// Room numbers, unique within the hostel. Blank entries are refused.
    pub room_numbers: Vec<String>,
}

/// Add rooms of one class, all or none, and return their ids in order.
#[instrument(skip(state, license_state), fields(command = "add_rooms"))]
pub async fn add_rooms(
    request: AddRoomsRequest,
    state: &DbPool,
    license_state: &crate::LicenseState,
    auth_state: &crate::AuthState,
) -> Result<Vec<i64>, String> {
    info!(
        hostel_id = request.hostel_id,
        count = request.room_numbers.len(),
        "add_rooms invoked"
    );
    require_license(license_state)?;
    // Rooms are hostel setup, which only an administrator changes.
    require_admin(auth_state, request.hostel_id)?;
    if request.room_numbers.is_empty() {
        return Err("Enter at least one room number.".to_string());
    }
    let mut rooms = Vec::with_capacity(request.room_numbers.len());
    for number in &request.room_numbers {
        let room_number = number.trim();
        if room_number.is_empty() {
            return Err("Room numbers cannot be blank.".to_string());
        }
        rooms.push(NewRoom {
            hostel_id: request.hostel_id,
            room_number: room_number.to_string(),
            class_id: request.class_id,
        });
    }
    state
        .add_rooms(&rooms)
        .await
        .map_err(|e| map_db_error("add_rooms", e))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::test_support::{setup_license_state, temp_db_url};

    #[tokio::test]
    async fn only_an_admin_sets_up_rooms() {
        let pool = DbPool::open(&temp_db_url()).await.expect("open database");
        let auth = crate::AuthState::new();
        auth.set_current_session(7, 1, "manager");
        let class = SaveRoomClassRequest {
            hostel_id: 1,
            class_id: None,
            floor: None,
            room_type: "single".to_string(),
            semester_rate: "450000".to_string(),
            recess_rate: "200000".to_string(),
        };
        let result = save_room_class(class, &pool, &setup_license_state(), &auth).await;
        assert_eq!(result, Err("forbidden".to_string()));
        let rooms = AddRoomsRequest {
            hostel_id: 1,
            class_id: 1,
            room_numbers: vec!["A01".to_string()],
        };
        let result = add_rooms(rooms, &pool, &setup_license_state(), &auth).await;
        assert_eq!(result, Err("forbidden".to_string()));
    }

    #[tokio::test]
    async fn get_rooms_returns_empty_for_unknown_hostel() {
        let url = temp_db_url();
        let pool = DbPool::open(&url).await.expect("open database");
        let rooms = pool.get_rooms(9999).await.expect("fetch rooms");
        assert!(rooms.is_empty());
    }
}
