use chrono::NaiveDate;
use db::{
    DbPool, Hostel, HostelProfile, NewAllocation, NewHostel, NewPayment, NewResident, NewRoom,
    NewStaffUser, Room, StaffUser,
};
use license::LicensePayload;
use rust_decimal::Decimal;
use serde::Deserialize;
use std::str::FromStr;
use tauri::{AppHandle, Manager, State};
use tracing::{error, info, instrument};

/// Payload for adding a new resident.
#[derive(Debug, Deserialize)]
pub struct AddResidentRequest {
    pub full_name: String,
    pub phone: Option<String>,
    pub email: Option<String>,
    pub id_number: Option<String>,
    pub next_of_kin: Option<String>,
    pub photo_path: Option<String>,
}

/// Payload for creating a room allocation.
#[derive(Debug, Deserialize)]
pub struct CreateAllocationRequest {
    pub hostel_id: i64,
    pub resident_id: i64,
    pub room_id: i64,
    pub bed_id: Option<i64>,
    pub check_in_date: NaiveDate,
    pub check_out_date: Option<NaiveDate>,
    pub expected_checkout: NaiveDate,
    pub rate_agreed: String,
}

/// Payload for recording a payment.
#[derive(Debug, Deserialize)]
pub struct RecordPaymentRequest {
    pub hostel_id: i64,
    pub allocation_id: i64,
    pub resident_id: i64,
    pub amount: String,
    pub payment_method: String,
    pub period_start: Option<NaiveDate>,
    pub period_end: Option<NaiveDate>,
    pub notes: Option<String>,
}

/// Payload for generating a PDF receipt.
#[derive(Debug, Deserialize)]
pub struct GenerateReceiptRequest {
    pub hostel_name: String,
    pub receipt_number: u64,
    pub resident_name: String,
    pub amount: String,
    pub date: NaiveDate,
    pub balance: String,
}

/// Payload for creating a staff login account.
#[derive(Debug, Deserialize)]
pub struct CreateStaffUserRequest {
    pub hostel_id: i64,
    pub name: String,
    pub username: String,
    pub password: String,
    pub role: String,
}

/// Payload for staff login.
#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

/// Fetch all rooms for a given hostel.
#[tauri::command]
#[instrument(skip(state), fields(command = "get_rooms"))]
pub async fn get_rooms(hostel_id: i64, state: State<'_, DbPool>) -> Result<Vec<Room>, String> {
    info!(hostel_id, "get_rooms invoked");
    state
        .get_rooms(hostel_id)
        .await
        .map_err(|e| map_db_error("get_rooms", e))
}

/// Add a new resident and return the generated resident id.
#[tauri::command]
#[instrument(skip(state), fields(command = "add_resident"))]
pub async fn add_resident(
    request: AddResidentRequest,
    state: State<'_, DbPool>,
) -> Result<i64, String> {
    info!(resident_name = %request.full_name, "add_resident invoked");

    let resident = NewResident {
        full_name: request.full_name,
        phone: request.phone,
        email: request.email,
        id_number: request.id_number,
        next_of_kin: request.next_of_kin,
        photo_path: request.photo_path,
    };

    state
        .add_resident(resident)
        .await
        .map_err(|e| map_db_error("add_resident", e))
}

/// Create a new allocation and return the generated allocation id.
#[tauri::command]
#[instrument(skip(state), fields(command = "create_allocation"))]
pub async fn create_allocation(
    request: CreateAllocationRequest,
    state: State<'_, DbPool>,
) -> Result<i64, String> {
    info!(
        hostel_id = request.hostel_id,
        resident_id = request.resident_id,
        room_id = request.room_id,
        "create_allocation invoked"
    );

    let rate_agreed = Decimal::from_str(&request.rate_agreed)
        .map_err(|_| "rate_agreed must be a valid decimal amount".to_string())?;

    let allocation = NewAllocation {
        hostel_id: request.hostel_id,
        resident_id: request.resident_id,
        room_id: request.room_id,
        bed_id: request.bed_id,
        check_in_date: request.check_in_date,
        check_out_date: request.check_out_date,
        expected_checkout: request.expected_checkout,
        rate_agreed: rate_agreed.to_string(),
    };

    state
        .create_allocation(allocation)
        .await
        .map_err(|e| {
            error!(error = %e, resident_id = request.resident_id, room_id = request.room_id, "create_allocation failed");
            format!("Could not create allocation. {e}")
        })
}

/// Record a payment and return the generated payment id.
#[tauri::command]
#[instrument(skip(state), fields(command = "record_payment"))]
pub async fn record_payment(
    request: RecordPaymentRequest,
    state: State<'_, DbPool>,
) -> Result<i64, String> {
    info!(
        hostel_id = request.hostel_id,
        allocation_id = request.allocation_id,
        resident_id = request.resident_id,
        "record_payment invoked"
    );

    let amount = Decimal::from_str(&request.amount)
        .map_err(|_| "amount must be a valid decimal amount".to_string())?;

    let payment = NewPayment {
        hostel_id: request.hostel_id,
        allocation_id: request.allocation_id,
        resident_id: request.resident_id,
        amount: amount.to_string(),
        payment_method: request.payment_method,
        period_start: request.period_start,
        period_end: request.period_end,
        notes: request.notes,
    };

    state.record_payment(payment).await.map_err(|e| {
        error!(error = %e, "record_payment failed");
        format!("Could not record payment. {e}")
    })
}

/// Record a payment and atomically generate the next sequential receipt number.
///
/// Returns the generated `payment_id` and `receipt_number`. The PDF itself is
/// generated separately via [`generate_and_open_receipt`].
#[tauri::command]
#[instrument(skip(state), fields(command = "record_payment_and_generate_receipt"))]
pub async fn record_payment_and_generate_receipt(
    request: RecordPaymentRequest,
    state: State<'_, DbPool>,
) -> Result<(i64, String), String> {
    info!(
        hostel_id = request.hostel_id,
        allocation_id = request.allocation_id,
        resident_id = request.resident_id,
        "record_payment_and_generate_receipt invoked"
    );

    let amount = Decimal::from_str(&request.amount)
        .map_err(|_| "amount must be a valid decimal amount".to_string())?;

    let payment = NewPayment {
        hostel_id: request.hostel_id,
        allocation_id: request.allocation_id,
        resident_id: request.resident_id,
        amount: amount.to_string(),
        payment_method: request.payment_method,
        period_start: request.period_start,
        period_end: request.period_end,
        notes: request.notes,
    };

    state
        .record_payment_and_receipt(payment)
        .await
        .map_err(|e| {
            error!(error = %e, "record_payment_and_generate_receipt failed");
            format!("Could not record payment and generate receipt. {e}")
        })
}

/// Generate a 30-day trial license signed with the test key.
///
/// Accepts an optional hostel name and returns a pasteable license string.
/// Defaults to "Trial Hostel" when no name is supplied.
#[tauri::command]
#[instrument(fields(command = "generate_trial_license"))]
pub async fn generate_trial_license(hostel_name: Option<String>) -> Result<String, String> {
    info!("generate_trial_license invoked");
    let name = hostel_name.unwrap_or_else(|| "Trial Hostel".to_string());
    Ok(license::generator::generate_trial_license(&name))
}

/// Generate a PDF receipt and open it with the default system PDF viewer.
#[tauri::command]
#[instrument(skip(app), fields(command = "generate_and_open_receipt"))]
pub async fn generate_and_open_receipt(
    request: GenerateReceiptRequest,
    app: tauri::AppHandle,
) -> Result<String, String> {
    info!(
        receipt_number = request.receipt_number,
        resident_name = %request.resident_name,
        "generate_and_open_receipt invoked"
    );

    let amount = Decimal::from_str(&request.amount)
        .map_err(|_| "amount must be a valid decimal amount".to_string())?;
    let balance = Decimal::from_str(&request.balance)
        .map_err(|_| "balance must be a valid decimal amount".to_string())?;

    let payload = pdf::ReceiptPayload {
        hostel_name: request.hostel_name,
        receipt_number: request.receipt_number,
        resident_name: request.resident_name,
        amount,
        date: request.date,
        balance,
    };

    let documents_dir = app
        .path()
        .document_dir()
        .map_err(|e| format!("Could not locate Documents folder: {e}"))?;
    let out_dir = documents_dir.join("HostelDesk").join("receipts");
    std::fs::create_dir_all(&out_dir).map_err(|e| e.to_string())?;

    let safe_name = payload
        .resident_name
        .replace(|c: char| !c.is_alphanumeric(), "_");
    let filename = format!(
        "receipt_{:08}_{}_{}.pdf",
        payload.receipt_number, safe_name, payload.date
    );
    let path = out_dir.join(filename);

    pdf::generate_receipt(&payload, &path).map_err(|e| e.to_string())?;
    open::that(&path).map_err(|e| e.to_string())?;

    Ok(path.to_string_lossy().to_string())
}

/// Return the currently active license payload, if any.
#[tauri::command]
#[instrument(skip(state))]
pub fn get_license_status(state: State<'_, crate::LicenseState>) -> Option<LicensePayload> {
    state.get()
}

/// Validate the supplied license string, persist it, and unlock the app.
#[tauri::command]
#[instrument(skip(state, app))]
pub fn activate_license(
    license: String,
    state: State<'_, crate::LicenseState>,
    app: AppHandle,
) -> Result<LicensePayload, String> {
    let payload = license::verify_license(&license)
        .map_err(|e| format!("License is invalid or expired: {e}"))?;

    let path = crate::license_path(&app)?;
    std::fs::write(&path, &license).map_err(|e| format!("could not save license: {e}"))?;
    info!(path = %path.display(), "license saved");

    state.set(payload.clone());
    Ok(payload)
}

/// Create a new staff user and return the generated user id.
#[tauri::command]
#[instrument(skip(state, request), fields(command = "create_staff_user"))]
pub async fn create_staff_user(
    request: CreateStaffUserRequest,
    state: State<'_, DbPool>,
) -> Result<i64, String> {
    info!(hostel_id = request.hostel_id, username = %request.username, "create_staff_user invoked");

    let password_hash = db::hash_password(&request.password)
        .map_err(|e| format!("Could not hash password: {e}"))?;

    let user = NewStaffUser {
        hostel_id: request.hostel_id,
        name: request.name,
        username: request.username,
        password_hash,
        role: request.role,
    };

    state
        .create_staff_user(user)
        .await
        .map_err(|e| map_db_error("create_staff_user", e))
}

/// Authenticate a staff user by username and password.
///
/// Returns the user record on success, or an error string when credentials are invalid.
#[tauri::command]
#[instrument(skip(state, request), fields(command = "login"))]
pub async fn login(request: LoginRequest, state: State<'_, DbPool>) -> Result<StaffUser, String> {
    info!(username = %request.username, "login invoked");

    state
        .authenticate_staff_user(&request.username, &request.password)
        .await
        .map_err(|e| map_db_error("login", e))?
        .ok_or_else(|| "Invalid username or password".to_string())
}

/// Convert an internal database error into a frontend-friendly string message.
fn map_db_error(command: &str, error: anyhow::Error) -> String {
    error!(command, %error, "command failed");
    format!("Something went wrong while processing your request. {error}")
}

/// Payload for creating a hostel during onboarding.
#[derive(Debug, Deserialize)]
pub struct CreateHostelRequest {
    pub name: String,
    pub address: Option<String>,
    pub phone: Option<String>,
    pub currency: Option<String>,
    pub branding_logo_path: Option<String>,
}

/// Payload for adding a single room.
#[derive(Debug, Deserialize)]
pub struct AddRoomRequest {
    pub hostel_id: i64,
    pub room_number: String,
    pub room_type: String,
    pub capacity: i32,
    pub floor: Option<i32>,
    /// Nightly/period rate as a decimal string, e.g. `"50.00"`.
    pub rate_per_period: String,
}

/// Create a hostel and return the full row including the assigned id.
#[tauri::command]
#[instrument(skip(state), fields(command = "create_hostel"))]
pub async fn create_hostel(
    request: CreateHostelRequest,
    state: State<'_, DbPool>,
) -> Result<Hostel, String> {
    info!(name = %request.name, "create_hostel invoked");
    if request.name.trim().is_empty() {
        return Err("hostel name is required".to_string());
    }
    let hostel = NewHostel {
        name: request.name,
        address: request.address,
        phone: request.phone,
        currency: request.currency,
        branding_logo_path: request.branding_logo_path,
    };
    state
        .create_hostel(hostel)
        .await
        .map_err(|e| map_db_error("create_hostel", e))
}

/// Add a single room and return the generated room id.
#[tauri::command]
#[instrument(skip(state), fields(command = "add_room"))]
pub async fn add_room(request: AddRoomRequest, state: State<'_, DbPool>) -> Result<i64, String> {
    info!(
        hostel_id = request.hostel_id,
        room_number = %request.room_number,
        "add_room invoked"
    );
    let rate = Decimal::from_str(&request.rate_per_period)
        .map_err(|_| "rate_per_period must be a valid decimal amount".to_string())?;
    let room = NewRoom {
        hostel_id: request.hostel_id,
        room_number: request.room_number,
        room_type: request.room_type,
        capacity: request.capacity,
        floor: request.floor,
        rate_per_period: rate.to_string(),
    };
    state
        .add_room(room)
        .await
        .map_err(|e| map_db_error("add_room", e))
}

/// Atomically insert many rooms; the whole batch rolls back on any failure.
#[tauri::command]
#[instrument(skip(state, requests), fields(command = "bulk_add_rooms"))]
pub async fn bulk_add_rooms(
    requests: Vec<AddRoomRequest>,
    state: State<'_, DbPool>,
) -> Result<Vec<i64>, String> {
    info!(count = requests.len(), "bulk_add_rooms invoked");
    let mut rooms = Vec::with_capacity(requests.len());
    for r in requests {
        let rate = Decimal::from_str(&r.rate_per_period)
            .map_err(|_| "rate_per_period must be a valid decimal amount".to_string())?;
        rooms.push(NewRoom {
            hostel_id: r.hostel_id,
            room_number: r.room_number,
            room_type: r.room_type,
            capacity: r.capacity,
            floor: r.floor,
            rate_per_period: rate.to_string(),
        });
    }
    state
        .bulk_add_rooms(rooms)
        .await
        .map_err(|e| map_db_error("bulk_add_rooms", e))
}

/// Return the hostel profile with derived onboarding state, or `None`.
#[tauri::command]
#[instrument(skip(state), fields(command = "get_hostel_profile"))]
pub async fn get_hostel_profile(
    hostel_id: i64,
    state: State<'_, DbPool>,
) -> Result<Option<HostelProfile>, String> {
    info!(hostel_id, "get_hostel_profile invoked");
    state
        .get_hostel_profile(hostel_id)
        .await
        .map_err(|e| map_db_error("get_hostel_profile", e))
}

/// Return all hostels ordered by id.
#[tauri::command]
#[instrument(skip(state), fields(command = "list_hostels"))]
pub async fn list_hostels(state: State<'_, DbPool>) -> Result<Vec<Hostel>, String> {
    info!("list_hostels invoked");
    state
        .list_hostels()
        .await
        .map_err(|e| map_db_error("list_hostels", e))
}
