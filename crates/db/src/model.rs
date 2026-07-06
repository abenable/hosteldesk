use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Room {
    pub id: i64,
    pub hostel_id: i64,
    pub room_number: String,
    pub room_type: String,
    pub capacity: i32,
    pub floor: Option<i32>,
    pub rate_per_period: String,
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
pub struct NewAllocation {
    pub hostel_id: i64,
    pub resident_id: i64,
    pub room_id: i64,
    pub bed_id: Option<i64>,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Allocation {
    pub id: i64,
    pub hostel_id: i64,
    pub resident_id: i64,
    pub room_id: i64,
    pub bed_id: Option<i64>,
    pub check_in_date: NaiveDate,
    pub check_out_date: Option<NaiveDate>,
    pub expected_checkout: NaiveDate,
    pub status: String,
    /// Rate agreed for this stay, stored as a decimal string.
    pub rate_agreed: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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
