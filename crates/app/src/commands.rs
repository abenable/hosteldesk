use chrono::NaiveDate;
use db::{
    ActiveAllocation, ActivityItem, AppPreferences, DashboardStats, DbPool, Hostel, HostelProfile,
    ListPaymentsFilter, NewCheckIn, NewHostel, NewPayment, NewRoom, NewUser, PaymentRow,
    ResidentDetail, ResidentSummary, Room, User, UpsertAppPreferences,
};
use hosteldesk_core::currency::{parse_money, parse_positive_money};
use license::LicensePayload;
use serde::Deserialize;
use sqlx::sqlite::SqliteConnectOptions;
use tauri::{AppHandle, Manager, State};
use tracing::{debug, error, info, instrument};

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
    pub photo_path: Option<String>,
    pub check_in_date: NaiveDate,
    pub check_out_date: Option<NaiveDate>,
    pub expected_checkout: NaiveDate,
    pub rate_agreed: String,
    /// `"semester_1"`, `"semester_2"` or `"recess"`.
    pub term: String,
    /// Academic year label, e.g. `"2026/2027"`.
    pub academic_year: String,
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
    pub hostel_id: i64,
    pub hostel_name: String,
    pub hostel_address: Option<String>,
    pub hostel_phone: Option<String>,
    pub receipt_number: u64,
    pub resident_name: String,
    pub room_number: String,
    pub amount: String,
    pub date: NaiveDate,
    pub balance: String,
    pub payment_method: String,
    pub notes: Option<String>,
    /// ISO 4217 currency code used to prefix monetary amounts.
    pub currency_code: String,
    /// Optional footer text rendered at the bottom of the receipt.
    pub footer_text: Option<String>,
    /// Optional path to a logo image to render at the top of the receipt.
    pub logo_path: Option<String>,
}

/// Payload for creating a staff login account.
#[derive(Debug, Deserialize)]
pub struct CreateUserRequest {
    pub hostel_id: i64,
    pub full_name: String,
    pub email: String,
    pub password: String,
    pub role: String,
}

/// Payload for staff login.
#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

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
    if !matches!(request.term.as_str(), "semester_1" | "semester_2" | "recess") {
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
        photo_path: request.photo_path,
        check_in_date: request.check_in_date,
        check_out_date: request.check_out_date,
        expected_checkout: request.expected_checkout,
        rate_agreed: rate_agreed.to_string(),
        term: request.term,
        academic_year: request.academic_year,
    };

    state.check_in(check_in).await.map_err(|e| {
        error!(error = %e, "check_in failed");
        format!("Could not check in resident. {e}")
    })
}

/// Record a payment and atomically generate the next sequential receipt number.
///
/// Returns the generated `payment_id` and `receipt_number`. The PDF itself is
/// generated separately via [`generate_and_open_receipt`].
#[tauri::command]
#[instrument(
    skip(state, license_state),
    fields(command = "record_payment_and_generate_receipt")
)]
pub async fn record_payment_and_generate_receipt(
    request: RecordPaymentRequest,
    state: State<'_, DbPool>,
    license_state: State<'_, crate::LicenseState>,
    auth_state: State<'_, crate::AuthState>,
) -> Result<(i64, String), String> {
    info!(
        hostel_id = request.hostel_id,
        allocation_id = request.allocation_id,
        resident_id = request.resident_id,
        "record_payment_and_generate_receipt invoked"
    );
    require_license(&license_state)?;
    require_session(&auth_state, request.hostel_id)?;

    let amount = parse_positive_money(&request.amount).map_err(|e| e.to_string())?;

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

/// Generate a PDF receipt, open it with the default system PDF viewer, and
/// persist the generated path back to the corresponding `receipts` row.
#[tauri::command]
#[instrument(
    skip(app, state, license_state),
    fields(command = "generate_and_open_receipt")
)]
pub async fn generate_and_open_receipt(
    request: GenerateReceiptRequest,
    app: tauri::AppHandle,
    state: State<'_, DbPool>,
    license_state: State<'_, crate::LicenseState>,
    auth_state: State<'_, crate::AuthState>,
) -> Result<String, String> {
    info!(
        receipt_number = request.receipt_number,
        resident_name = %request.resident_name,
        "generate_and_open_receipt invoked"
    );
    require_license(&license_state)?;
    require_session(&auth_state, request.hostel_id)?;

    let amount = parse_positive_money(&request.amount).map_err(|e| e.to_string())?;
    let balance = parse_money(&request.balance).map_err(|e| e.to_string())?;
    let logo_path = validate_logo_path(&app, request.hostel_id, request.logo_path)?;

    let payload = pdf::ReceiptPayload {
        hostel_name: request.hostel_name,
        hostel_address: request.hostel_address,
        hostel_phone: request.hostel_phone,
        receipt_number: request.receipt_number,
        resident_name: request.resident_name,
        room_number: request.room_number,
        amount,
        date: request.date,
        balance,
        payment_method: request.payment_method,
        notes: request.notes,
        currency_code: request.currency_code,
        footer_text: request.footer_text,
        logo_path,
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

    let path_str = path.to_string_lossy().to_string();
    let receipt_number = request.receipt_number.to_string();
    if let Err(e) = state
        .update_receipt_pdf_path(request.hostel_id, &receipt_number, &path_str)
        .await
    {
        debug!(error = %e, hostel_id = request.hostel_id, receipt_number, "failed to persist receipt pdf_path");
    }

    Ok(path_str)
}

/// Return the currently active license payload, if any.
#[tauri::command]
#[instrument(skip(state))]
pub fn get_license_status(state: State<'_, crate::LicenseState>) -> Option<LicensePayload> {
    state.get()
}

/// Validate a license string, persist it, and update in-memory state. Shared
/// by [`activate_license`] and [`activate_license_from_file`].
fn activate_license_string(
    license: &str,
    state: &State<'_, crate::LicenseState>,
    app: &AppHandle,
) -> Result<LicensePayload, String> {
    let payload = license::verify_license(license)
        .map_err(|e| format!("License is invalid or expired: {e}"))?;

    let path = crate::license_path(app)?;
    std::fs::write(&path, license).map_err(|e| format!("could not save license: {e}"))?;
    info!(path = %path.display(), "license saved");

    state.set(payload.clone());
    Ok(payload)
}

/// Open a native "open file" dialog for a license file, then validate and
/// persist it.
#[tauri::command]
#[instrument(skip(state, app))]
pub fn activate_license_from_file(
    state: State<'_, crate::LicenseState>,
    app: AppHandle,
) -> Result<LicensePayload, String> {
    let path = rfd::FileDialog::new()
        .set_title("Select your HostelDesk license file")
        .add_filter("HostelDesk license", &["hdlicense", "lic", "txt"])
        .pick_file()
        .ok_or_else(|| "No file selected".to_string())?;

    let contents = std::fs::read_to_string(&path)
        .map_err(|e| format!("could not read license file: {e}"))?;

    activate_license_string(contents.trim(), &state, &app)
}

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

/// Return the running application version (`CARGO_PKG_VERSION`), shown in
/// Settings so a user has something to quote in a bug report.
#[tauri::command]
pub fn get_app_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
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

/// Return the staff user represented by the current backend session.
#[tauri::command]
#[instrument(skip(state, auth_state), fields(command = "get_session"))]
pub async fn get_session(
    state: State<'_, DbPool>,
    auth_state: State<'_, crate::AuthState>,
) -> Result<Option<User>, String> {
    info!("get_session invoked");
    let Some(session) = auth_state.current_session() else {
        return Ok(None);
    };

    let user = state
        .get_user_by_id(session.user_id)
        .await
        .map_err(|e| map_db_error("get_session", e))?;
    if user.is_none() {
        auth_state.clear_current_session();
    }
    Ok(user)
}

/// Clear the current backend session.
#[tauri::command]
#[instrument(skip(auth_state), fields(command = "logout"))]
pub fn logout(auth_state: State<'_, crate::AuthState>) {
    info!("logout invoked");
    auth_state.clear_current_session();
}

/// Create a new staff user and return the generated user id.
#[tauri::command]
#[instrument(
    skip(state, request, license_state, auth_state),
    fields(command = "create_user")
)]
pub async fn create_user(
    request: CreateUserRequest,
    state: State<'_, DbPool>,
    license_state: State<'_, crate::LicenseState>,
    auth_state: State<'_, crate::AuthState>,
) -> Result<i64, String> {
    info!(hostel_id = request.hostel_id, email = %request.email, "create_user invoked");
    require_license(&license_state)?;
    require_admin(&auth_state, request.hostel_id)?;
    validate_password(&request.password)?;

    let password_hash = db::hash_password(&request.password)
        .map_err(|e| format!("Could not hash password: {e}"))?;

    let user = NewUser {
        hostel_id: request.hostel_id,
        full_name: request.full_name,
        email: request.email,
        password_hash,
        role: request.role,
    };

    state
        .create_user(user)
        .await
        .map_err(|e| map_db_error("create_user", e))
}

/// Authenticate a staff user by email and password.
///
/// On success, mints the authoritative in-memory session and returns the user
/// record for display. The returned role is never used for authorization.
#[tauri::command]
#[instrument(skip(state, auth_state, request), fields(command = "login"))]
pub async fn login(
    request: LoginRequest,
    state: State<'_, DbPool>,
    license_state: State<'_, crate::LicenseState>,
    auth_state: State<'_, crate::AuthState>,
) -> Result<User, String> {
    info!(email = %request.email, "login invoked");
    require_license(&license_state)?;

    let user = state
        .authenticate_user(&request.email, &request.password)
        .await
        .map_err(|e| map_db_error("login", e))?
        .ok_or_else(|| "Invalid email or password".to_string())?;

    auth_state.set_current_session(user.id, user.hostel_id, user.role.clone());
    Ok(user)
}

/// Convert an internal database error into a frontend-friendly string message.
fn map_db_error(command: &str, error: anyhow::Error) -> String {
    error!(command, %error, "command failed");
    format!("Something went wrong while processing your request. {error}")
}

/// Confirm a caller-supplied logo path actually points inside this hostel's
/// own upload directory (`<app_data_dir>/hostels/<hostel_id>/`) before it is
/// handed to the PDF renderer to read from disk.
///
/// `logo_path` normally round-trips the value [`upload_hostel_logo`] already
/// wrote, but it arrives over IPC as a plain string, so nothing stops a
/// caller from pointing it at an arbitrary file elsewhere on disk. Mirrors
/// `restore_backup`'s containment check: canonicalize both sides and fail
/// closed rather than pass an unvalidated path through.
fn validate_logo_path(
    app: &tauri::AppHandle,
    hostel_id: i64,
    logo_path: Option<String>,
) -> Result<Option<String>, String> {
    let Some(logo_path) = logo_path else {
        return Ok(None);
    };

    let data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("could not resolve app data dir: {e}"))?;
    let logo_dir = data_dir.join("hostels").join(hostel_id.to_string());

    let candidate = std::path::Path::new(&logo_path)
        .canonicalize()
        .map_err(|_| "logo path could not be read".to_string())?;
    let logo_dir = logo_dir
        .canonicalize()
        .map_err(|_| "logo path could not be read".to_string())?;

    if !candidate.starts_with(&logo_dir) {
        return Err("logo path is outside the hostel's logo directory".to_string());
    }

    Ok(Some(candidate.to_string_lossy().to_string()))
}

/// Enforce the password policy server-side. The 8-character minimum shown in
/// `TeamSection.tsx` is a UX hint only — without this, `create_user` /
/// `update_user` accept any password, since the backend never re-checks it.
fn validate_password(password: &str) -> Result<(), String> {
    if password.chars().count() < 8 {
        return Err("Password must be at least 8 characters.".to_string());
    }
    Ok(())
}

/// Validate an academic year label like `"2026/2027"`: two 4-digit years
/// separated by a slash. Not stricter than that (e.g. year ordering) — it's a
/// free-text label, not a calculated value.
fn is_academic_year(value: &str) -> bool {
    let Some((a, b)) = value.split_once('/') else {
        return false;
    };
    a.len() == 4 && b.len() == 4 && a.bytes().all(|c| c.is_ascii_digit()) && b.bytes().all(|c| c.is_ascii_digit())
}

/// Guard commands that require a valid, unexpired license.
fn require_license(license_state: &crate::LicenseState) -> Result<LicensePayload, String> {
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
fn require_session(
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

/// Return the current session without comparing it to a request target.
fn require_session_current(auth_state: &crate::AuthState) -> Result<crate::Session, String> {
    auth_state
        .current_session()
        .ok_or_else(|| "unauthorized".to_string())
}

/// Guard app-wide commands that affect the current hostel's database.
///
/// Backup and restore have no target-hostel request because they operate on the
/// single application database, so the session's own hostel is the target.
fn require_admin_current(auth_state: &crate::AuthState) -> Result<crate::Session, String> {
    let session = auth_state
        .current_session()
        .ok_or_else(|| "unauthorized".to_string())?;
    if session.role != "admin" {
        return Err("forbidden".to_string());
    }
    Ok(session)
}

/// Guard admin-only commands for the target hostel.
fn require_admin(auth_state: &crate::AuthState, hostel_id: i64) -> Result<crate::Session, String> {
    let session = require_session(auth_state, hostel_id)?;
    if session.role != "admin" {
        return Err("forbidden".to_string());
    }
    Ok(session)
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

    let semester_rate =
        parse_positive_money(&request.semester_rate).map_err(|e| e.to_string())?;
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

/// Request payload for [`check_out`].
#[derive(Debug, Deserialize)]
pub struct CheckOutRequest {
    pub hostel_id: i64,
    pub allocation_id: i64,
    pub check_out_date: NaiveDate,
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
        .check_out(
            request.hostel_id,
            request.allocation_id,
            request.check_out_date,
        )
        .await
        .map_err(|e| map_db_error("check_out", e))
}

/// List the payments ledger, optionally filtered by date, resident or method.
#[tauri::command]
#[instrument(skip(state, license_state), fields(command = "list_payments"))]
pub async fn list_payments(
    request: ListPaymentsFilter,
    state: State<'_, DbPool>,
    license_state: State<'_, crate::LicenseState>,
    auth_state: State<'_, crate::AuthState>,
) -> Result<Vec<PaymentRow>, String> {
    info!(hostel_id = request.hostel_id, "list_payments invoked");
    require_license(&license_state)?;
    require_session(&auth_state, request.hostel_id)?;
    state
        .list_payments(request)
        .await
        .map_err(|e| map_db_error("list_payments", e))
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

/// Request payload for [`get_dashboard_stats`].
#[derive(Debug, Deserialize)]
pub struct GetDashboardStatsRequest {
    pub hostel_id: i64,
    pub today: NaiveDate,
}

/// Return dashboard statistics for a hostel on the supplied day.
#[tauri::command]
#[instrument(skip(state, license_state), fields(command = "get_dashboard_stats"))]
pub async fn get_dashboard_stats(
    request: GetDashboardStatsRequest,
    state: State<'_, DbPool>,
    license_state: State<'_, crate::LicenseState>,
    auth_state: State<'_, crate::AuthState>,
) -> Result<DashboardStats, String> {
    info!(
        hostel_id = request.hostel_id,
        today = %request.today,
        "get_dashboard_stats invoked"
    );
    require_license(&license_state)?;
    require_session(&auth_state, request.hostel_id)?;
    state
        .get_dashboard_stats(request.hostel_id, request.today)
        .await
        .map_err(|e| map_db_error("get_dashboard_stats", e))
}

/// Request payload for [`list_recent_activity`].
#[derive(Debug, Deserialize)]
pub struct ListRecentActivityRequest {
    pub hostel_id: i64,
    pub limit: i64,
}

/// Return recent check-ins, check-outs and payments for the activity feed.
#[tauri::command]
#[instrument(skip(state, license_state), fields(command = "list_recent_activity"))]
pub async fn list_recent_activity(
    request: ListRecentActivityRequest,
    state: State<'_, DbPool>,
    license_state: State<'_, crate::LicenseState>,
    auth_state: State<'_, crate::AuthState>,
) -> Result<Vec<ActivityItem>, String> {
    info!(
        hostel_id = request.hostel_id,
        "list_recent_activity invoked"
    );
    require_license(&license_state)?;
    require_session(&auth_state, request.hostel_id)?;
    state
        .list_recent_activity(request.hostel_id, request.limit)
        .await
        .map_err(|e| map_db_error("list_recent_activity", e))
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
        })
        .await
        .map_err(|e| map_db_error("update_resident", e))
}

/// Request payload for [`delete_user`].
#[derive(Debug, Deserialize)]
pub struct DeleteUserRequest {
    pub user_id: i64,
}

/// Request payload for [`list_users`].
#[derive(Debug, Deserialize)]
pub struct ListUsersRequest {
    pub hostel_id: i64,
}

/// List staff users for a hostel.
#[tauri::command]
#[instrument(skip(state, license_state), fields(command = "list_users"))]
pub async fn list_users(
    request: ListUsersRequest,
    state: State<'_, DbPool>,
    license_state: State<'_, crate::LicenseState>,
    auth_state: State<'_, crate::AuthState>,
) -> Result<Vec<User>, String> {
    info!(hostel_id = request.hostel_id, "list_users invoked");
    require_license(&license_state)?;
    require_admin(&auth_state, request.hostel_id)?;
    state
        .list_users(request.hostel_id)
        .await
        .map_err(|e| map_db_error("list_users", e))
}

/// Request payload for [`update_user`].
#[derive(Debug, Deserialize)]
pub struct UpdateUserRequest {
    pub user_id: i64,
    pub full_name: String,
    pub email: String,
    pub role: String,
    pub password: Option<String>,
}

/// Update a staff user's profile and optionally their password.
#[tauri::command]
#[instrument(
    skip(state, request, license_state, auth_state),
    fields(command = "update_user")
)]
pub async fn update_user(
    request: UpdateUserRequest,
    state: State<'_, DbPool>,
    license_state: State<'_, crate::LicenseState>,
    auth_state: State<'_, crate::AuthState>,
) -> Result<User, String> {
    info!(user_id = request.user_id, "update_user invoked");
    require_license(&license_state)?;
    let target = state
        .get_user_by_id(request.user_id)
        .await
        .map_err(|e| map_db_error("update_user", e))?
        .ok_or_else(|| "user not found".to_string())?;
    let session = require_session(&auth_state, target.hostel_id)?;
    let role_changed = target.role != request.role;
    // Changing any role, or editing someone else's account, requires admin.
    // A non-admin may still update their own name/email/password as long as
    // their role stays the same — this is how a manager rotates their own
    // password (AccountSection.tsx), which a blanket self-update admin gate
    // used to block.
    if session.role != "admin" && (role_changed || request.user_id != session.user_id) {
        return Err("forbidden".to_string());
    }

    let password_hash = match request.password {
        Some(ref password) => {
            validate_password(password)?;
            Some(db::hash_password(password).map_err(|e| format!("Could not hash password: {e}"))?)
        }
        None => None,
    };

    state
        .update_user(
            request.user_id,
            request.full_name,
            request.email,
            request.role,
            password_hash,
        )
        .await
        .map_err(|e| map_db_error("update_user", e))
}

/// Delete a staff user, refusing to remove the last admin of a hostel.
#[tauri::command]
#[instrument(
    skip(state, request, license_state, auth_state),
    fields(command = "delete_user")
)]
pub async fn delete_user(
    request: DeleteUserRequest,
    state: State<'_, DbPool>,
    license_state: State<'_, crate::LicenseState>,
    auth_state: State<'_, crate::AuthState>,
) -> Result<(), String> {
    info!(user_id = request.user_id, "delete_user invoked");
    require_license(&license_state)?;
    let target = state
        .get_user_by_id(request.user_id)
        .await
        .map_err(|e| map_db_error("delete_user", e))?
        .ok_or_else(|| "user not found".to_string())?;
    require_admin(&auth_state, target.hostel_id)?;
    state
        .delete_user(request.user_id)
        .await
        .map_err(|e| map_db_error("delete_user", e))
}

/// Back up the live database into a timestamped file under `Documents/HostelDesk/backups/`.
///
/// Uses SQLite `VACUUM INTO` to produce a compact, consistent single-file copy
/// of the database. Safe to run on a live WAL database — no write lock is held
/// on the source.
///
/// Returns the absolute path to the created backup file.
#[tauri::command]
#[instrument(
    skip(app, state, license_state, auth_state),
    fields(command = "backup_database")
)]
pub async fn backup_database(
    app: AppHandle,
    state: State<'_, DbPool>,
    license_state: State<'_, crate::LicenseState>,
    auth_state: State<'_, crate::AuthState>,
) -> Result<String, String> {
    info!("backup_database invoked");
    require_license(&license_state)?;
    require_admin_current(&auth_state)?;

    let documents_dir = app
        .path()
        .document_dir()
        .map_err(|e| format!("Could not locate Documents folder: {e}"))?;
    let backup_dir = documents_dir.join("HostelDesk").join("backups");
    std::fs::create_dir_all(&backup_dir)
        .map_err(|e| format!("Could not create backup directory: {e}"))?;

    let now = chrono::Local::now();
    let filename = format!("hosteldesk-{}.db", now.format("%Y%m%d-%H%M%S"));
    let backup_path = backup_dir.join(filename);
    let path_str = backup_path.to_string_lossy().to_string();

    sqlx::query("VACUUM INTO ?1")
        .bind(&path_str)
        .execute(&state.inner)
        .await
        .map_err(|e| {
            error!(error = %e, "backup_database failed");
            format!("Could not create backup: {e}")
        })?;

    info!(path = %backup_path.display(), "backup created");
    Ok(path_str)
}

/// List timestamped database backups in `Documents/HostelDesk/backups/`.
#[tauri::command]
#[instrument(skip(app, license_state), fields(command = "list_backups"))]
pub async fn list_backups(
    app: AppHandle,
    license_state: State<'_, crate::LicenseState>,
    auth_state: State<'_, crate::AuthState>,
) -> Result<Vec<String>, String> {
    info!("list_backups invoked");
    require_license(&license_state)?;
    require_admin_current(&auth_state)?;

    let backup_dir = documents_hosteldesk_dir(&app)?.join("backups");
    if !backup_dir.exists() {
        return Ok(Vec::new());
    }

    let mut entries: Vec<_> = std::fs::read_dir(&backup_dir)
        .map_err(|e| format!("could not read backups directory: {e}"))?
        .filter_map(|entry| entry.ok())
        .filter(|entry| {
            entry
                .path()
                .extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| ext.eq_ignore_ascii_case("db"))
                .unwrap_or(false)
        })
        .collect();

    entries.sort_by_key(|entry| {
        entry
            .metadata()
            .and_then(|m| m.modified())
            .unwrap_or(std::time::SystemTime::UNIX_EPOCH)
    });
    entries.reverse();

    let paths: Vec<String> = entries
        .into_iter()
        .map(|entry| entry.path().to_string_lossy().to_string())
        .collect();

    Ok(paths)
}

/// Request payload for [`restore_backup`].
#[derive(Debug, Deserialize)]
pub struct RestoreBackupRequest {
    pub backup_path: String,
}

/// Restore a backup file over the live database and restart the app.
///
/// The `backup_path` is validated to live inside `Documents/HostelDesk/backups/`
/// to prevent arbitrary file overwrites. The database pool is closed before the
/// copy so the replacement is clean.
#[tauri::command]
#[instrument(
    skip(app, state, license_state, auth_state),
    fields(command = "restore_backup")
)]
pub async fn restore_backup(
    request: RestoreBackupRequest,
    app: AppHandle,
    state: State<'_, DbPool>,
    license_state: State<'_, crate::LicenseState>,
    auth_state: State<'_, crate::AuthState>,
) -> Result<(), String> {
    info!(backup_path = %request.backup_path, "restore_backup invoked");
    require_license(&license_state)?;
    require_admin_current(&auth_state)?;

    let backup_dir = documents_hosteldesk_dir(&app)?.join("backups");
    let backup_path = std::path::Path::new(&request.backup_path)
        .canonicalize()
        .map_err(|e| format!("invalid backup path: {e}"))?;
    let backup_dir = backup_dir.canonicalize().unwrap_or(backup_dir);

    if !backup_path.starts_with(&backup_dir) {
        return Err("backup file must be inside the HostelDesk backups folder".to_string());
    }

    let db_path = database_path(&app)?;
    let db_path_str = db_path.to_string_lossy().to_string();
    let wal_path = std::path::PathBuf::from(format!("{}-wal", db_path_str));
    let shm_path = std::path::PathBuf::from(format!("{}-shm", db_path_str));
    let pre_restore_path = db_path.with_extension("db.pre-restore");

    validate_backup_database(&backup_path).await?;

    // Build a consistent safety copy before closing the pool. VACUUM INTO is
    // safe with WAL mode and captures all committed transactions.
    if pre_restore_path.exists() {
        std::fs::remove_file(&pre_restore_path)
            .map_err(|e| format!("could not clear pre-restore snapshot: {e}"))?;
    }
    let pre_restore_str = pre_restore_path.to_string_lossy().to_string();
    sqlx::query("VACUUM INTO ?1")
        .bind(&pre_restore_str)
        .execute(&state.inner)
        .await
        .map_err(|e| format!("could not create pre-restore snapshot: {e}"))?;

    // Close the pool so the live database file is no longer held open.
    state.inner.close().await;

    if let Err(e) = replace_database_files(&db_path, &backup_path, &wal_path, &shm_path) {
        error!(error = %e, "database restore failed; recovering pre-restore snapshot");
        let _ = std::fs::remove_file(&db_path);
        if let Err(copy_err) = std::fs::copy(&pre_restore_path, &db_path) {
            error!(
                error = %copy_err,
                "CRITICAL: could not recover pre-restore snapshot after failed restore"
            );
            return Err(format!(
                "Restore failed ({e}) and the automatic recovery copy also failed \
                 ({copy_err}). Your database from before the restore attempt is still \
                 safe at {}. Do not restart HostelDesk yet; copy that file back \
                 manually or contact support.",
                pre_restore_path.display()
            ));
        }
        let _ = std::fs::remove_file(&wal_path);
        let _ = std::fs::remove_file(&shm_path);

        info!("database recovered from pre-restore snapshot; restarting application");
        app.restart();
    }

    info!("database restored; restarting application");
    app.restart();
}

/// Validate that a backup is a readable, consistent SQLite database.
///
/// This runs before the live pool is closed so a rejected backup can never leave
/// the application in a partially restored state.
async fn validate_backup_database(path: &std::path::Path) -> Result<(), String> {
    let options = SqliteConnectOptions::new()
        .filename(path)
        .read_only(true)
        .create_if_missing(false);
    let pool = sqlx::SqlitePool::connect_with(options)
        .await
        .map_err(|e| format!("not a valid SQLite backup: {e}"))?;

    let check: String = sqlx::query_scalar("PRAGMA quick_check")
        .fetch_one(&pool)
        .await
        .map_err(|e| format!("not a valid SQLite backup: {e}"))?;
    pool.close().await;

    if check != "ok" {
        return Err(format!(
            "not a valid SQLite backup: quick_check returned {check}"
        ));
    }
    Ok(())
}

/// Replace database files while retaining a recovery snapshot.
///
/// The caller has already validated `backup_path`. If any step fails after the
/// live pool has closed, the snapshot is copied back so the next launch opens a
/// working database.
fn replace_database_files(
    db_path: &std::path::Path,
    backup_path: &std::path::Path,
    wal_path: &std::path::Path,
    shm_path: &std::path::Path,
) -> Result<(), String> {
    let pre_restore_path = db_path.with_extension("db.pre-restore");
    let staging_path = db_path.with_extension("db.restoring");

    // The staging file is a leftover from a previous crashed restore attempt,
    // if it exists at all; a fresh restore never has one yet.
    let _ = std::fs::remove_file(&staging_path);

    std::fs::copy(backup_path, &staging_path)
        .map_err(|e| format!("could not stage backup for restore: {e}"))?;

    let _ = std::fs::remove_file(wal_path);
    let _ = std::fs::remove_file(shm_path);
    std::fs::remove_file(db_path)
        .map_err(|e| format!("could not remove live database for restore: {e}"))?;

    if let Err(e) = std::fs::rename(&staging_path, db_path) {
        let _ = std::fs::copy(&pre_restore_path, db_path);
        let _ = std::fs::remove_file(&staging_path);
        return Err(format!("could not install restored database: {e}"));
    }

    let _ = std::fs::remove_file(wal_path);
    let _ = std::fs::remove_file(shm_path);
    Ok(())
}

/// Resolve the user's HostelDesk documents directory.
fn documents_hosteldesk_dir(app: &AppHandle) -> Result<std::path::PathBuf, String> {
    let documents_dir = app
        .path()
        .document_dir()
        .map_err(|e| format!("Could not locate Documents folder: {e}"))?;
    Ok(documents_dir.join("HostelDesk"))
}

/// Return the path to the live database file.
fn database_path(app: &AppHandle) -> Result<std::path::PathBuf, String> {
    let data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("could not resolve app data dir: {e}"))?;
    Ok(data_dir.join("hosteldesk.db"))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn backup_validation_rejects_non_sqlite_file() {
        let dir = tempfile::tempdir().expect("create temp dir");
        let path = dir.path().join("not-a-backup.db");
        std::fs::write(&path, b"definitely not sqlite").expect("write invalid backup");

        let err = validate_backup_database(&path)
            .await
            .expect_err("invalid backup should be rejected");
        assert!(
            err.contains("not a valid SQLite backup"),
            "unexpected validation error: {err}"
        );
    }

    /// Regression test for R2: a fresh restore has no leftover `.db.restoring`
    /// staging file, and `replace_database_files` must not fail just because
    /// there is nothing to clear.
    #[tokio::test]
    async fn replace_database_files_succeeds_with_no_stale_staging_file() {
        let dir = tempfile::tempdir().expect("create temp dir");
        let db_path = dir.path().join("hosteldesk.db");
        let backup_path = dir.path().join("backup.db");
        let wal_path = dir.path().join("hosteldesk.db-wal");
        let shm_path = dir.path().join("hosteldesk.db-shm");

        std::fs::write(&db_path, b"live database contents").expect("write live db");
        std::fs::write(&backup_path, b"backup database contents").expect("write backup");
        std::fs::write(&wal_path, b"stale wal").expect("write wal");
        std::fs::write(&shm_path, b"stale shm").expect("write shm");

        replace_database_files(&db_path, &backup_path, &wal_path, &shm_path)
            .expect("restore should succeed when no staging file is left over");

        let restored = std::fs::read(&db_path).expect("read restored db");
        assert_eq!(restored, b"backup database contents");
        assert!(!wal_path.exists(), "stale wal sidecar should be removed");
        assert!(!shm_path.exists(), "stale shm sidecar should be removed");
    }

    /// A failed restore (backup file missing) must leave the live database
    /// file untouched and report an error instead of destroying live data.
    #[tokio::test]
    async fn replace_database_files_leaves_live_db_untouched_when_backup_unreadable() {
        let dir = tempfile::tempdir().expect("create temp dir");
        let db_path = dir.path().join("hosteldesk.db");
        let missing_backup_path = dir.path().join("does-not-exist.db");
        let wal_path = dir.path().join("hosteldesk.db-wal");
        let shm_path = dir.path().join("hosteldesk.db-shm");

        std::fs::write(&db_path, b"live database contents").expect("write live db");

        let err = replace_database_files(&db_path, &missing_backup_path, &wal_path, &shm_path)
            .expect_err("restore from a missing backup file must fail");
        assert!(
            err.contains("could not stage backup for restore"),
            "unexpected error: {err}"
        );

        let live = std::fs::read(&db_path).expect("live db must still exist");
        assert_eq!(
            live, b"live database contents",
            "live database must be untouched when staging the backup fails"
        );
    }
}
