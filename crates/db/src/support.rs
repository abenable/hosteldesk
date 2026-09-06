use chrono::Local;
use chrono::NaiveDate;
use chrono::TimeZone;
use chrono::Utc;
use rust_decimal::Decimal;

/// Return the UTC bounds of a local calendar day.
///
/// Payments are stored as UTC timestamps, while operators select dates in the
/// hostel's local timezone. The exclusive upper bound lets SQLite use the
/// `payments(hostel_id, payment_date)` index without wrapping the indexed column
/// in `date()` or `strftime()`.
///
/// Bounds are formatted as `%Y-%m-%dT%H:%M:%SZ` to match `payments.payment_date`'s
/// column default, so bytewise TEXT comparison in SQL agrees with chronological
/// order (see migration `20260905000000_initial_schema.sql`).
pub(crate) fn local_day_bounds(date: NaiveDate) -> anyhow::Result<(String, String)> {
    let start_naive = date
        .and_hms_opt(0, 0, 0)
        .ok_or_else(|| anyhow::anyhow!("invalid local date start: {date}"))?;
    let end_naive = (date + chrono::Duration::days(1))
        .and_hms_opt(0, 0, 0)
        .ok_or_else(|| anyhow::anyhow!("invalid local date end: {date}"))?;
    let start = Local
        .from_local_datetime(&start_naive)
        .earliest()
        .ok_or_else(|| anyhow::anyhow!("invalid local date start: {date}"))?;
    let end = Local
        .from_local_datetime(&end_naive)
        .earliest()
        .ok_or_else(|| anyhow::anyhow!("invalid local date end: {date}"))?;
    const FMT: &str = "%Y-%m-%dT%H:%M:%SZ";
    Ok((
        start.with_timezone(&Utc).format(FMT).to_string(),
        end.with_timezone(&Utc).format(FMT).to_string(),
    ))
}

/// Sum a comma-separated list of decimal amounts produced by SQLite's
/// `GROUP_CONCAT`. Returns zero for `None` or an empty string.
///
/// Each amount is a stored `payments.amount` value, which the schema's CHECK
/// constraint already guarantees is non-negative — `parse_money` here is
/// defense-in-depth, not the only thing standing between a negative amount
/// and a balance calculation.
pub(crate) fn sum_decimal_amounts(amounts: Option<&str>) -> anyhow::Result<Decimal> {
    match amounts {
        None | Some("") => Ok(Decimal::ZERO),
        Some(s) => {
            let values: Vec<Decimal> = s
                .split(',')
                .map(|v| hosteldesk_core::currency::parse_money(v.trim()))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(values.iter().copied().fold(Decimal::ZERO, |acc, d| acc + d))
        }
    }
}

/// Compute the outstanding balance for an allocation: a flat one-time sale of
/// `rate_agreed` (one agreed term, e.g. one academic semester) minus what has
/// been paid. Not a per-day accrual — a stay that runs long is a front-desk
/// problem (check the resident out, or open a new allocation for the next
/// term), not something this formula bills extra for.
pub(crate) fn allocation_balance(
    rate_agreed: &str,
    total_paid: Decimal,
) -> anyhow::Result<Decimal> {
    let rate = hosteldesk_core::currency::parse_money(rate_agreed)?;
    Ok(rate - total_paid)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::NewPayment;
    use crate::test_support::*;
    use std::str::FromStr;

    #[tokio::test]
    async fn decimal_amounts_sum_exactly_without_float_drift() {
        let pool = test_pool().await;
        let hostel = seed_hostel(&pool).await;
        let room_id = seed_room(&pool, hostel.id, "101", "single").await;
        let resident_id = seed_resident(&pool, "Ivy").await;
        let allocation_id = seed_active_allocation(&pool, hostel.id, resident_id, room_id).await;

        for amount in ["0.10", "0.20"] {
            pool.record_payment_and_receipt(NewPayment {
                hostel_id: hostel.id,
                allocation_id,
                resident_id,
                amount: amount.to_string(),
                payment_method: "cash".to_string(),
                period_start: None,
                period_end: None,
                notes: None,
            })
            .await
            .expect("record");
        }

        let residents = pool.list_residents(hostel.id).await.expect("list");
        let ivy = residents
            .iter()
            .find(|r| r.full_name == "Ivy")
            .expect("ivy");
        assert_eq!(ivy.balance, "99.70");

        let total = sum_decimal_amounts(Some("0.10,0.20")).expect("sum");
        assert_eq!(total, Decimal::from_str("0.30").unwrap());
    }
}
