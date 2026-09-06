use super::support::*;
use chrono::NaiveDate;
use db::{DbPool, ListPaymentsFilter, NewPayment, PaymentRow};
use hosteldesk_core::currency::{parse_money, parse_positive_money};
use serde::Deserialize;
use tauri::Manager;
use tauri::State;
use tracing::{debug, error, info, instrument};

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::test_support::{setup_license_state, temp_db_url};
    use crate::{commands, AuthState, LicenseState};
    use tauri::Manager;

    async fn setup_payment_fixture(pool: &DbPool) -> (i64, i64, i64, i64) {
        let hostel = pool
            .create_hostel(db::NewHostel {
                name: "Money Test Hostel".to_string(),
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
                full_name: Some("Money Test Resident".to_string()),
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
        (hostel.id, room_id, resident_id, allocation_id)
    }

    #[tokio::test]
    async fn record_payment_and_generate_receipt_rejects_negative_amount() {
        let url = temp_db_url();
        let pool = DbPool::open(&url).await.expect("open database");
        let (hostel_id, _room_id, resident_id, allocation_id) = setup_payment_fixture(&pool).await;
        let license_state = setup_license_state();

        let app = tauri::test::mock_app();
        app.manage(pool);
        app.manage(license_state);
        app.manage(AuthState::new());

        let request = commands::RecordPaymentRequest {
            hostel_id,
            allocation_id,
            resident_id,
            amount: "-1".to_string(),
            payment_method: "cash".to_string(),
            period_start: None,
            period_end: None,
            notes: None,
        };

        let result = commands::record_payment_and_generate_receipt(
            request,
            app.state::<DbPool>(),
            app.state::<LicenseState>(),
            app.state::<AuthState>(),
        )
        .await;
        assert!(result.is_err(), "negative amount should be rejected");

        let payment_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM payments")
            .fetch_one(&app.state::<DbPool>().inner)
            .await
            .expect("count payments");
        assert_eq!(payment_count, 0, "no payment row should be inserted");

        let receipt_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM receipts")
            .fetch_one(&app.state::<DbPool>().inner)
            .await
            .expect("count receipts");
        assert_eq!(receipt_count, 0, "no receipt row should be inserted");
    }

    #[tokio::test]
    async fn record_payment_and_generate_receipt_rejects_zero_amount() {
        let url = temp_db_url();
        let pool = DbPool::open(&url).await.expect("open database");
        let (hostel_id, _room_id, resident_id, allocation_id) = setup_payment_fixture(&pool).await;
        let license_state = setup_license_state();

        let app = tauri::test::mock_app();
        app.manage(pool);
        app.manage(license_state);
        app.manage(AuthState::new());

        let request = commands::RecordPaymentRequest {
            hostel_id,
            allocation_id,
            resident_id,
            amount: "0".to_string(),
            payment_method: "cash".to_string(),
            period_start: None,
            period_end: None,
            notes: None,
        };

        let result = commands::record_payment_and_generate_receipt(
            request,
            app.state::<DbPool>(),
            app.state::<LicenseState>(),
            app.state::<AuthState>(),
        )
        .await;
        assert!(result.is_err(), "zero amount should be rejected");

        let payment_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM payments")
            .fetch_one(&app.state::<DbPool>().inner)
            .await
            .expect("count payments");
        assert_eq!(payment_count, 0, "no payment row should be inserted");

        let receipt_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM receipts")
            .fetch_one(&app.state::<DbPool>().inner)
            .await
            .expect("count receipts");
        assert_eq!(receipt_count, 0, "no receipt row should be inserted");
    }

    #[tokio::test]
    async fn payments_check_constraint_rejects_negative_amount() {
        let url = temp_db_url();
        let pool = DbPool::open(&url).await.expect("open database");
        let (hostel_id, _room_id, resident_id, allocation_id) = setup_payment_fixture(&pool).await;

        let err = sqlx::query(
            "INSERT INTO payments (hostel_id, allocation_id, resident_id, amount, payment_method)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        )
        .bind(hostel_id)
        .bind(allocation_id)
        .bind(resident_id)
        .bind("-1.00")
        .bind("cash")
        .execute(&pool.inner)
        .await
        .expect_err("negative amount should violate the CHECK constraint");
        assert!(
            err.to_string().contains("CHECK") || err.to_string().contains("constraint"),
            "error should be a CHECK constraint failure: {err}"
        );
    }

    #[tokio::test]
    async fn payments_check_constraint_rejects_zero_amount() {
        let url = temp_db_url();
        let pool = DbPool::open(&url).await.expect("open database");
        let (hostel_id, _room_id, resident_id, allocation_id) = setup_payment_fixture(&pool).await;

        let err = sqlx::query(
            "INSERT INTO payments (hostel_id, allocation_id, resident_id, amount, payment_method)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        )
        .bind(hostel_id)
        .bind(allocation_id)
        .bind(resident_id)
        .bind("0.00")
        .bind("cash")
        .execute(&pool.inner)
        .await
        .expect_err("zero amount should violate the CHECK constraint");
        assert!(
            err.to_string().contains("CHECK") || err.to_string().contains("constraint"),
            "error should be a CHECK constraint failure: {err}"
        );
    }
}
