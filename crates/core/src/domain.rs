//! Pure domain types for Hostel Desk.
//!
//! These types have no dependency on SQL, Tauri, or UI frameworks. They are
//! meant to be constructed from frontend payloads and validated before any
//! persistence step.

use crate::currency::MoneyError;
use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Errors that can occur while validating domain values.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ValidationError {
    /// A required field was empty or missing.
    #[error("{0} is required")]
    MissingField(&'static str),
    /// A numeric value was out of range.
    #[error("{0} must be greater than zero")]
    MustBePositive(&'static str),
    /// A date range was invalid.
    #[error("{0} must be on or after {1}")]
    InvalidDateRange(&'static str, &'static str),
    /// A categorical value was not one of the allowed choices.
    #[error("{0} is not a valid value")]
    InvalidValue(&'static str),
    /// A monetary amount could not be parsed or violated its constraints.
    #[error(transparent)]
    Money(#[from] MoneyError),
}

/// Status of a room or bed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RoomStatus {
    /// Available for allocation.
    Vacant,
    /// Currently allocated.
    Occupied,
    /// Unavailable due to maintenance.
    Maintenance,
}

impl RoomStatus {
    /// Returns the string stored in the database.
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Vacant => "vacant",
            Self::Occupied => "occupied",
            Self::Maintenance => "maintenance",
        }
    }
}

/// Type of room offered by a hostel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RoomType {
    /// Single occupant room.
    Single,
    /// Shared dormitory room with multiple beds.
    Shared,
    /// Private room, potentially with multiple beds for one party.
    Private,
}

impl RoomType {
    /// Returns the string stored in the database.
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Single => "single",
            Self::Shared => "shared",
            Self::Private => "private",
        }
    }
}

/// Status of a resident allocation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AllocationStatus {
    /// Resident is currently checked in.
    Active,
    /// Resident has checked out.
    CheckedOut,
}

impl AllocationStatus {
    /// Returns the string stored in the database.
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::CheckedOut => "checked_out",
        }
    }
}

/// Method used to record a payment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PaymentMethod {
    /// Cash.
    Cash,
    /// Mobile money (MTN/Airtel Money, etc.).
    MobileMoney,
    /// Bank transfer.
    BankTransfer,
    /// Card payment.
    Card,
}

impl PaymentMethod {
    /// Returns the string stored in the database.
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Cash => "cash",
            Self::MobileMoney => "mobile_money",
            Self::BankTransfer => "bank_transfer",
            Self::Card => "card",
        }
    }
}

/// A hostel property managed by the application.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Hostel {
    /// Database identifier.
    pub id: i64,
    /// Display name of the hostel.
    pub name: String,
    /// Physical address, if recorded.
    pub address: Option<String>,
    /// Contact phone number, if recorded.
    pub phone: Option<String>,
    /// ISO 4217 currency code used for receipts and reporting.
    pub currency: String,
    /// Optional path to a logo file for branded receipts.
    pub branding_logo_path: Option<String>,
}

impl Hostel {
    /// Validate the hostel fields that must be present before persistence.
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.name.trim().is_empty() {
            return Err(ValidationError::MissingField("name"));
        }
        if self.currency.trim().is_empty() {
            return Err(ValidationError::MissingField("currency"));
        }
        Ok(())
    }
}

/// Input for creating a new hostel.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NewHostel {
    /// Display name of the hostel.
    pub name: String,
    /// Physical address, if recorded.
    pub address: Option<String>,
    /// Contact phone number, if recorded.
    pub phone: Option<String>,
    /// ISO 4217 currency code used for receipts and reporting.
    pub currency: String,
    /// Optional path to a logo file for branded receipts.
    pub branding_logo_path: Option<String>,
}

impl NewHostel {
    /// Validate the hostel fields that must be present before persistence.
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.name.trim().is_empty() {
            return Err(ValidationError::MissingField("name"));
        }
        if self.currency.trim().is_empty() {
            return Err(ValidationError::MissingField("currency"));
        }
        Ok(())
    }
}

/// A room in a hostel.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Room {
    /// Database identifier.
    pub id: i64,
    /// Owning hostel identifier.
    pub hostel_id: i64,
    /// Room number or label (e.g. "101").
    pub room_number: String,
    /// Type of room.
    pub room_type: RoomType,
    /// Maximum number of occupants.
    pub capacity: i32,
    /// Floor number, if recorded.
    pub floor: Option<i32>,
    /// Standard rate charged per billing period.
    pub rate_per_period: Decimal,
    /// Current room status.
    pub status: RoomStatus,
}

impl Room {
    /// Validate room fields before persistence.
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.room_number.trim().is_empty() {
            return Err(ValidationError::MissingField("room_number"));
        }
        if self.capacity <= 0 {
            return Err(ValidationError::MustBePositive("capacity"));
        }
        if self.rate_per_period.is_sign_negative() || self.rate_per_period.is_zero() {
            return Err(ValidationError::MustBePositive("rate_per_period"));
        }
        Ok(())
    }
}

/// Input for creating a new room.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NewRoom {
    /// Owning hostel identifier.
    pub hostel_id: i64,
    /// Room number or label.
    pub room_number: String,
    /// Type of room.
    pub room_type: RoomType,
    /// Maximum number of occupants.
    pub capacity: i32,
    /// Floor number, if recorded.
    pub floor: Option<i32>,
    /// Standard rate charged per billing period.
    pub rate_per_period: Decimal,
    /// Initial room status.
    pub status: RoomStatus,
}

impl NewRoom {
    /// Validate room fields before persistence.
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.room_number.trim().is_empty() {
            return Err(ValidationError::MissingField("room_number"));
        }
        if self.capacity <= 0 {
            return Err(ValidationError::MustBePositive("capacity"));
        }
        if self.rate_per_period.is_sign_negative() || self.rate_per_period.is_zero() {
            return Err(ValidationError::MustBePositive("rate_per_period"));
        }
        Ok(())
    }
}

/// A bed inside a shared dorm room.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Bed {
    /// Database identifier.
    pub id: i64,
    /// Owning room identifier.
    pub room_id: i64,
    /// Bed label (e.g. "A", "lower").
    pub bed_label: String,
    /// Current bed status.
    pub status: RoomStatus,
}

impl Bed {
    /// Validate bed fields before persistence.
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.bed_label.trim().is_empty() {
            return Err(ValidationError::MissingField("bed_label"));
        }
        Ok(())
    }
}

/// Input for creating a new bed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NewBed {
    /// Owning room identifier.
    pub room_id: i64,
    /// Bed label.
    pub bed_label: String,
    /// Initial bed status.
    pub status: RoomStatus,
}

impl NewBed {
    /// Validate bed fields before persistence.
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.bed_label.trim().is_empty() {
            return Err(ValidationError::MissingField("bed_label"));
        }
        Ok(())
    }
}

/// A resident staying at the hostel.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Resident {
    /// Database identifier.
    pub id: i64,
    /// Full name of the resident.
    pub full_name: String,
    /// Contact phone number.
    pub phone: Option<String>,
    /// Email address.
    pub email: Option<String>,
    /// Government or organizational ID number.
    pub id_number: Option<String>,
    /// Emergency contact information.
    pub next_of_kin: Option<String>,
    /// Optional path to a resident photo.
    pub photo_path: Option<String>,
}

impl Resident {
    /// Validate resident fields before persistence.
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.full_name.trim().is_empty() {
            return Err(ValidationError::MissingField("full_name"));
        }
        Ok(())
    }
}

/// Input for creating a resident.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NewResident {
    /// Full name of the resident.
    pub full_name: String,
    /// Contact phone number.
    pub phone: Option<String>,
    /// Email address.
    pub email: Option<String>,
    /// Government or organizational ID number.
    pub id_number: Option<String>,
    /// Emergency contact information.
    pub next_of_kin: Option<String>,
    /// Optional path to a resident photo.
    pub photo_path: Option<String>,
}

impl NewResident {
    /// Validate resident fields before persistence.
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.full_name.trim().is_empty() {
            return Err(ValidationError::MissingField("full_name"));
        }
        Ok(())
    }
}

/// A room allocation linking a resident to a room (and optionally a bed).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Allocation {
    /// Database identifier.
    pub id: i64,
    /// Owning hostel identifier.
    pub hostel_id: i64,
    /// Resident allocated to the room.
    pub resident_id: i64,
    /// Room allocated to the resident.
    pub room_id: i64,
    /// Specific bed, if this is a shared dorm.
    pub bed_id: Option<i64>,
    /// Date the resident checked in.
    pub check_in_date: NaiveDate,
    /// Actual check-out date, if the resident has left.
    pub check_out_date: Option<NaiveDate>,
    /// Expected check-out date used for reminders and billing.
    pub expected_checkout: NaiveDate,
    /// Current allocation status.
    pub status: AllocationStatus,
    /// Rate agreed for this stay.
    pub rate_agreed: Decimal,
}

impl Allocation {
    /// Validate allocation fields before persistence.
    pub fn validate(&self) -> Result<(), ValidationError> {
        validate_dates(
            self.check_in_date,
            self.check_out_date,
            self.expected_checkout,
        )?;
        if self.rate_agreed.is_sign_negative() || self.rate_agreed.is_zero() {
            return Err(ValidationError::MustBePositive("rate_agreed"));
        }
        Ok(())
    }
}

/// Input for creating a new allocation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NewAllocation {
    /// Owning hostel identifier.
    pub hostel_id: i64,
    /// Resident allocated to the room.
    pub resident_id: i64,
    /// Room allocated to the resident.
    pub room_id: i64,
    /// Specific bed, if this is a shared dorm.
    pub bed_id: Option<i64>,
    /// Date the resident checks in.
    pub check_in_date: NaiveDate,
    /// Actual check-out date, if already known.
    pub check_out_date: Option<NaiveDate>,
    /// Expected check-out date used for reminders and billing.
    pub expected_checkout: NaiveDate,
    /// Rate agreed for this stay.
    pub rate_agreed: Decimal,
}

impl NewAllocation {
    /// Validate allocation fields before persistence.
    pub fn validate(&self) -> Result<(), ValidationError> {
        validate_dates(
            self.check_in_date,
            self.check_out_date,
            self.expected_checkout,
        )?;
        if self.rate_agreed.is_sign_negative() || self.rate_agreed.is_zero() {
            return Err(ValidationError::MustBePositive("rate_agreed"));
        }
        Ok(())
    }
}

/// A payment recorded against an allocation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Payment {
    /// Database identifier.
    pub id: i64,
    /// Owning hostel identifier.
    pub hostel_id: i64,
    /// Allocation this payment is for.
    pub allocation_id: i64,
    /// Resident who made the payment.
    pub resident_id: i64,
    /// Amount paid.
    pub amount: Decimal,
    /// Method used to make the payment.
    pub payment_method: PaymentMethod,
    /// Start of the billing period covered, if applicable.
    pub period_start: Option<NaiveDate>,
    /// End of the billing period covered, if applicable.
    pub period_end: Option<NaiveDate>,
    /// Free-form notes about the payment.
    pub notes: Option<String>,
}

impl Payment {
    /// Validate payment fields before persistence.
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.amount.is_sign_negative() || self.amount.is_zero() {
            return Err(ValidationError::MustBePositive("amount"));
        }
        validate_period(self.period_start, self.period_end)?;
        Ok(())
    }
}

/// Input for recording a payment.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NewPayment {
    /// Owning hostel identifier.
    pub hostel_id: i64,
    /// Allocation this payment is for.
    pub allocation_id: i64,
    /// Resident who made the payment.
    pub resident_id: i64,
    /// Amount paid.
    pub amount: Decimal,
    /// Method used to make the payment (e.g. "cash", "mobile_money").
    pub payment_method: String,
    /// Start of the billing period covered, if applicable.
    pub period_start: Option<NaiveDate>,
    /// End of the billing period covered, if applicable.
    pub period_end: Option<NaiveDate>,
    /// Free-form notes about the payment.
    pub notes: Option<String>,
}

impl NewPayment {
    /// Validate payment fields before persistence.
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.amount.is_sign_negative() || self.amount.is_zero() {
            return Err(ValidationError::MustBePositive("amount"));
        }
        validate_payment_method(&self.payment_method)?;
        validate_period(self.period_start, self.period_end)?;
        Ok(())
    }
}

/// Validate that a checkout date range is internally consistent.
fn validate_dates(
    check_in: NaiveDate,
    check_out: Option<NaiveDate>,
    expected_checkout: NaiveDate,
) -> Result<(), ValidationError> {
    if let Some(check_out) = check_out {
        if check_out < check_in {
            return Err(ValidationError::InvalidDateRange(
                "check_out_date",
                "check_in_date",
            ));
        }
    }
    if expected_checkout < check_in {
        return Err(ValidationError::InvalidDateRange(
            "expected_checkout",
            "check_in_date",
        ));
    }
    Ok(())
}

/// Validate that a payment method is one of the accepted values.
fn validate_payment_method(method: &str) -> Result<(), ValidationError> {
    let method = method.trim();
    if method.is_empty() {
        return Err(ValidationError::MissingField("payment_method"));
    }
    match method {
        "cash" | "mobile_money" | "bank_transfer" | "card" => Ok(()),
        _ => Err(ValidationError::InvalidValue("payment_method")),
    }
}

/// Validate that a billing period end is not before its start.
fn validate_period(
    period_start: Option<NaiveDate>,
    period_end: Option<NaiveDate>,
) -> Result<(), ValidationError> {
    if let (Some(start), Some(end)) = (period_start, period_end) {
        if end < start {
            return Err(ValidationError::InvalidDateRange(
                "period_end",
                "period_start",
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    fn date(year: i32, month: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(year, month, day).unwrap()
    }

    #[test]
    fn new_resident_requires_name() {
        let resident = NewResident {
            full_name: "   ".to_string(),
            phone: None,
            email: None,
            id_number: None,
            next_of_kin: None,
            photo_path: None,
        };
        assert!(matches!(
            resident.validate(),
            Err(ValidationError::MissingField("full_name"))
        ));
    }

    #[test]
    fn new_resident_accepts_valid_name() {
        let resident = NewResident {
            full_name: "Jane Doe".to_string(),
            phone: Some("+123".to_string()),
            email: None,
            id_number: None,
            next_of_kin: None,
            photo_path: None,
        };
        assert!(resident.validate().is_ok());
    }

    #[test]
    fn new_allocation_validates_rate_and_dates() {
        let check_in = date(2025, 1, 1);
        let mut allocation = NewAllocation {
            hostel_id: 1,
            resident_id: 2,
            room_id: 3,
            bed_id: None,
            check_in_date: check_in,
            check_out_date: None,
            expected_checkout: date(2025, 1, 31),
            rate_agreed: Decimal::from_str("300.00").unwrap(),
        };
        assert!(allocation.validate().is_ok());

        allocation.rate_agreed = Decimal::ZERO;
        assert!(allocation.validate().is_err());

        allocation.rate_agreed = Decimal::from_str("300.00").unwrap();
        allocation.expected_checkout = date(2024, 12, 31);
        assert!(matches!(
            allocation.validate(),
            Err(ValidationError::InvalidDateRange(
                "expected_checkout",
                "check_in_date"
            ))
        ));
    }

    #[test]
    fn new_payment_validates_amount_method_and_period() {
        let mut payment = NewPayment {
            hostel_id: 1,
            allocation_id: 2,
            resident_id: 3,
            amount: Decimal::from_str("150.00").unwrap(),
            payment_method: "cash".to_string(),
            period_start: Some(date(2025, 1, 1)),
            period_end: Some(date(2025, 1, 31)),
            notes: None,
        };
        assert!(payment.validate().is_ok());

        payment.amount = Decimal::from_str("-10.00").unwrap();
        assert!(payment.validate().is_err());

        payment.amount = Decimal::from_str("150.00").unwrap();
        payment.payment_method = "crypto".to_string();
        assert!(matches!(
            payment.validate(),
            Err(ValidationError::InvalidValue("payment_method"))
        ));

        payment.payment_method = "cash".to_string();
        payment.period_end = Some(date(2024, 12, 31));
        assert!(matches!(
            payment.validate(),
            Err(ValidationError::InvalidDateRange(
                "period_end",
                "period_start"
            ))
        ));
    }

    #[test]
    fn room_requires_positive_capacity_and_rate() {
        let room = Room {
            id: 1,
            hostel_id: 1,
            room_number: "101".to_string(),
            room_type: RoomType::Single,
            capacity: 0,
            floor: Some(1),
            rate_per_period: Decimal::from_str("100.00").unwrap(),
            status: RoomStatus::Vacant,
        };
        assert!(matches!(
            room.validate(),
            Err(ValidationError::MustBePositive("capacity"))
        ));

        let mut room = Room {
            capacity: 2,
            ..room
        };
        room.rate_per_period = Decimal::ZERO;
        assert!(matches!(
            room.validate(),
            Err(ValidationError::MustBePositive("rate_per_period"))
        ));
    }

    #[test]
    fn enums_serialize_to_database_values() {
        assert_eq!(RoomStatus::Occupied.as_str(), "occupied");
        assert_eq!(RoomType::Shared.as_str(), "shared");
        assert_eq!(AllocationStatus::Active.as_str(), "active");
        assert_eq!(PaymentMethod::MobileMoney.as_str(), "mobile_money");
    }
}
