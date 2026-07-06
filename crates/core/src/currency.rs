//! Currency parsing and validation.
//!
//! Hostel Desk never uses floating-point numbers for money. All monetary
//! values are represented as [`Decimal`] and stored in SQLite as TEXT.

use rust_decimal::Decimal;
use std::str::FromStr;
use thiserror::Error;

/// Errors that can occur while parsing or validating a monetary amount.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum MoneyError {
    /// The input is empty or not a valid decimal number.
    #[error("amount must be a valid decimal amount")]
    InvalidDecimal,
    /// The parsed amount is negative. Negative rates and payments are never valid.
    #[error("amount must not be negative")]
    Negative,
    /// The parsed amount is zero where a positive value is required.
    #[error("amount must be greater than zero")]
    Zero,
}

/// Parse a non-negative currency amount from a string.
///
/// Leading and trailing whitespace is ignored. Returns an error if the input
/// is empty, malformed, or negative. Zero is accepted; callers that require a
/// strictly positive amount can use [`parse_positive_money`].
pub fn parse_money(amount: &str) -> Result<Decimal, MoneyError> {
    let decimal = Decimal::from_str(amount.trim()).map_err(|_| MoneyError::InvalidDecimal)?;
    if decimal.is_sign_negative() {
        return Err(MoneyError::Negative);
    }
    Ok(decimal)
}

/// Parse a strictly positive currency amount from a string.
///
/// Use this for rates and payment amounts, both of which must be greater than
/// zero.
pub fn parse_positive_money(amount: &str) -> Result<Decimal, MoneyError> {
    let decimal = parse_money(amount)?;
    if decimal.is_zero() {
        return Err(MoneyError::Zero);
    }
    Ok(decimal)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_valid_amounts() {
        assert_eq!(
            parse_money("150.50").unwrap(),
            Decimal::from_str("150.50").unwrap()
        );
        assert_eq!(
            parse_money("  49.50  ").unwrap(),
            Decimal::from_str("49.50").unwrap()
        );
        assert_eq!(parse_money("0.00").unwrap(), Decimal::ZERO);
    }

    #[test]
    fn rejects_negative_amounts() {
        assert_eq!(parse_money("-10.00"), Err(MoneyError::Negative));
    }

    #[test]
    fn rejects_invalid_decimals() {
        assert_eq!(parse_money(""), Err(MoneyError::InvalidDecimal));
        assert_eq!(parse_money("abc"), Err(MoneyError::InvalidDecimal));
        assert_eq!(parse_money("12.34.56"), Err(MoneyError::InvalidDecimal));
    }

    #[test]
    fn rejects_zero_for_positive() {
        assert_eq!(parse_positive_money("0.00"), Err(MoneyError::Zero));
        assert_eq!(parse_positive_money("0"), Err(MoneyError::Zero));
    }

    #[test]
    fn accepts_positive_for_positive() {
        assert_eq!(
            parse_positive_money("0.01").unwrap(),
            Decimal::from_str("0.01").unwrap()
        );
    }
}
