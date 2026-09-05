use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

/// Per-hostel application preferences.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct AppPreferences {
    pub hostel_id: i64,
    pub date_format: String,
    pub language: String,
    pub receipt_footer: Option<String>,
    pub updated_at: DateTime<Utc>,
}

/// Data required to create or update app preferences.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpsertAppPreferences {
    pub hostel_id: i64,
    pub date_format: String,
    pub language: String,
    pub receipt_footer: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Room {
    pub id: i64,
    pub hostel_id: i64,
    pub room_number: String,
    pub room_type: String,
    pub capacity: i32,
    pub floor: Option<i32>,
    pub rate_per_period: String,
    /// Billing period for this room (`night`, `week`, `month`, `semester`).
    pub billing_period: String,
    pub status: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewResident {
    pub full_name: String,
    pub phone: Option<String>,
    pub email: Option<String>,
    pub id_number: Option<String>,
    pub next_of_kin: Option<String>,
    pub photo_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
/// Data required to create a room allocation.
pub struct NewAllocation {
    pub hostel_id: i64,
    pub resident_id: i64,
    pub room_id: i64,
    pub check_in_date: NaiveDate,
    pub check_out_date: Option<NaiveDate>,
    pub expected_checkout: NaiveDate,
    pub rate_agreed: String,
}

/// Data required to check a resident into a room in one atomic transaction.
///
/// Either `resident_id` (returning resident) or `full_name` (new resident)
/// must be supplied. `rate_agreed` is a decimal string already validated by
/// the caller.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewCheckIn {
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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewPayment {
    pub hostel_id: i64,
    pub allocation_id: i64,
    pub resident_id: i64,
    /// Payment amount as a decimal string.
    pub amount: String,
    pub payment_method: String,
    pub period_start: Option<NaiveDate>,
    pub period_end: Option<NaiveDate>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Resident {
    pub id: i64,
    pub full_name: String,
    pub phone: Option<String>,
    pub email: Option<String>,
    pub id_number: Option<String>,
    pub next_of_kin: Option<String>,
    pub photo_path: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Allocation {
    pub id: i64,
    pub hostel_id: i64,
    pub resident_id: i64,
    pub room_id: i64,
    pub check_in_date: NaiveDate,
    pub check_out_date: Option<NaiveDate>,
    pub expected_checkout: NaiveDate,
    pub status: String,
    /// Rate agreed for this stay, stored as a decimal string.
    pub rate_agreed: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Payment {
    pub id: i64,
    pub hostel_id: i64,
    pub allocation_id: i64,
    pub resident_id: i64,
    /// Payment amount as a decimal string.
    pub amount: String,
    pub payment_date: DateTime<Utc>,
    pub payment_method: String,
    pub period_start: Option<NaiveDate>,
    pub period_end: Option<NaiveDate>,
    pub notes: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// A hostel record. One row per licensed property.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Hostel {
    pub id: i64,
    pub name: String,
    pub address: Option<String>,
    pub phone: Option<String>,
    pub currency: String,
    pub branding_logo_path: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// Data required to create a hostel during onboarding.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewHostel {
    pub name: String,
    pub address: Option<String>,
    pub phone: Option<String>,
    pub currency: Option<String>,
    pub branding_logo_path: Option<String>,
}

/// Data required to complete first-run onboarding atomically.
///
/// The included hostel is created only when no hostel exists; the first staff
/// account is always created with the `admin` role.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompleteOnboarding {
    /// Hostel profile to create.
    pub hostel: NewHostel,
    /// Display name for the first administrator.
    pub admin_name: String,
    /// Login username for the first administrator.
    pub username: String,
    /// PHC password hash for the first administrator.
    pub password_hash: String,
}

/// Data required to create a single room.
///
/// `status` is deliberately absent — new rooms are always created `vacant`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewRoom {
    pub hostel_id: i64,
    pub room_number: String,
    pub room_type: String,
    pub capacity: i32,
    pub floor: Option<i32>,
    /// Nightly/period rate as a decimal string, e.g. `"50.00"`.
    pub rate_per_period: String,
    /// Billing period (`night`, `week`, `month`, `semester`).
    /// `None` defaults to `"semester"` on insert.
    pub billing_period: Option<String>,
}

/// Hostel profile augmented with derived onboarding state.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct HostelProfile {
    pub id: i64,
    pub name: String,
    pub address: Option<String>,
    pub phone: Option<String>,
    pub currency: String,
    pub branding_logo_path: Option<String>,
    pub room_count: i64,
    pub admin_count: i64,
    /// True when the hostel has at least one room and one admin staff user.
    pub onboarding_complete: bool,
}

/// A staff user account, used for authentication responses.
///
/// The `password_hash` is deliberately omitted; it should never leave the DB layer.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct StaffUser {
    pub id: i64,
    pub hostel_id: i64,
    pub name: String,
    pub username: String,
    pub role: String,
    pub created_at: DateTime<Utc>,
}

/// Data required to create a new staff user record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewStaffUser {
    pub hostel_id: i64,
    pub name: String,
    pub username: String,
    /// PHC-formatted password hash produced by [`hash_password`](crate::hash_password).
    pub password_hash: String,
    pub role: String,
}

/// Summary of a resident shown in the directory list.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ResidentSummary {
    pub id: i64,
    pub full_name: String,
    pub phone: Option<String>,
    pub email: Option<String>,
    /// Current active allocation id, if any.
    pub allocation_id: Option<i64>,
    pub room_number: Option<String>,
    pub check_in_date: Option<NaiveDate>,
    pub expected_checkout: Option<NaiveDate>,
    /// `"active"`, `"checked_out"` or `"never_allocated"`.
    pub status: String,
    /// Owed minus paid for the active allocation, or `"0"`.
    pub balance: String,
}

/// An allocation augmented with the room number it belongs to.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct AllocationWithRoom {
    pub id: i64,
    pub hostel_id: i64,
    pub resident_id: i64,
    pub room_id: i64,
    pub check_in_date: NaiveDate,
    pub check_out_date: Option<NaiveDate>,
    pub expected_checkout: NaiveDate,
    /// `"active"` or `"checked_out"`.
    pub status: String,
    /// Rate agreed for this stay, stored as a decimal string.
    pub rate_agreed: String,
    pub created_at: DateTime<Utc>,
    pub room_number: String,
    /// Billing period of the room (`night`, `week`, `month`, `semester`) used to
    /// compute accrued balance. Included here so balance math can use the room's
    /// period without an extra lookup.
    pub billing_period: String,
}

/// Full resident record with stay and payment history.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResidentDetail {
    pub resident: Resident,
    pub allocations: Vec<AllocationWithRoom>,
    pub payments: Vec<Payment>,
    /// Net balance owed across all of the resident's allocations in the hostel.
    pub balance: String,
}

/// One row in the payments ledger.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct PaymentRow {
    pub id: i64,
    pub resident_id: i64,
    pub resident_name: String,
    pub allocation_id: i64,
    pub room_number: String,
    pub amount: String,
    pub payment_method: String,
    pub payment_date: DateTime<Utc>,
    pub receipt_number: Option<String>,
    pub notes: Option<String>,
}

/// Filter criteria for the payments ledger.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListPaymentsFilter {
    pub hostel_id: i64,
    pub from: Option<NaiveDate>,
    pub to: Option<NaiveDate>,
    pub resident_id: Option<i64>,
    pub method: Option<String>,
}

/// An active allocation shown in the payment-form resident picker.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ActiveAllocation {
    pub allocation_id: i64,
    pub resident_id: i64,
    pub resident_name: String,
    pub room_number: String,
    pub rate_agreed: String,
    pub check_in_date: NaiveDate,
    pub expected_checkout: NaiveDate,
    /// Total paid so far, as a decimal string.
    pub total_paid: String,
    /// Current outstanding balance, as a decimal string.
    pub balance: String,
}

/// Dashboard statistics for a hostel on a given day.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct DashboardStats {
    pub total_rooms: i64,
    pub occupied: i64,
    pub vacant: i64,
    pub maintenance: i64,
    pub check_ins_today: i64,
    pub departures_today: i64,
    pub overdue_stays: i64,
    pub revenue_today: String,
    pub revenue_month: String,
    pub outstanding_total: String,
}

/// A single recent activity entry for the dashboard feed.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ActivityItem {
    /// `"check_in"`, `"check_out"` or `"payment"`.
    pub kind: String,
    pub at: DateTime<Utc>,
    pub resident_name: String,
    pub room_number: String,
    /// Payment amount, only set for payment items.
    pub amount: Option<String>,
}
