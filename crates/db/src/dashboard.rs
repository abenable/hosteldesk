use crate::model::*;
use crate::support::{allocation_balance, local_day_bounds, sum_decimal_amounts};
use crate::DbPool;
use chrono::Datelike;
use chrono::NaiveDate;
use rust_decimal::Decimal;
use sqlx::Row;
use tracing::{debug, error, instrument};

impl DbPool {
    /// Return dashboard statistics for a hostel on a specific day.
    ///
    /// `today` is supplied by the caller so the dashboard reflects the user's
    /// local calendar day.
    #[instrument(skip(self))]
    pub async fn get_dashboard_stats(
        &self,
        hostel_id: i64,
        today: NaiveDate,
    ) -> anyhow::Result<DashboardStats> {
        debug!(hostel_id, %today, "fetching dashboard stats");

        let counts = sqlx::query(
            r#"
            SELECT
                (SELECT COUNT(*) FROM rooms WHERE hostel_id = ?1) AS total_rooms,
                (SELECT COUNT(*) FROM rooms WHERE hostel_id = ?1 AND status = 'occupied') AS occupied,
                (SELECT COUNT(*) FROM rooms WHERE hostel_id = ?1 AND status = 'vacant') AS vacant,
                (SELECT COUNT(*) FROM rooms WHERE hostel_id = ?1 AND status = 'maintenance') AS maintenance,
                (SELECT COUNT(*) FROM allocations WHERE hostel_id = ?1 AND DATE(created_at) = ?2) AS check_ins_today
            "#,
        )
        .bind(hostel_id)
        .bind(today)
        .fetch_one(&self.inner)
        .await
        .inspect_err(|e| error!(error = %e, hostel_id, "database error fetching dashboard counts"))?;

        let revenue_today = self.revenue_for_date(hostel_id, today).await?;
        let revenue_month = self.revenue_for_month(hostel_id, today).await?;
        let outstanding_total = self.outstanding_balance_total(hostel_id).await?;

        Ok(DashboardStats {
            total_rooms: counts.try_get("total_rooms")?,
            occupied: counts.try_get("occupied")?,
            vacant: counts.try_get("vacant")?,
            maintenance: counts.try_get("maintenance")?,
            check_ins_today: counts.try_get("check_ins_today")?,
            revenue_today: revenue_today.to_string(),
            revenue_month: revenue_month.to_string(),
            outstanding_total: outstanding_total.to_string(),
        })
    }

    /// Return recent check-ins, check-outs and payments for the dashboard feed.
    #[instrument(skip(self))]
    pub async fn list_recent_activity(
        &self,
        hostel_id: i64,
        limit: i64,
    ) -> anyhow::Result<Vec<ActivityItem>> {
        debug!(hostel_id, limit, "listing recent activity");
        let limit = limit.min(50);
        let check_ins = sqlx::query_as::<_, ActivityItem>(
            r#"
            SELECT 'check_in' AS kind,
                   a.created_at AS at,
                   r.full_name AS resident_name,
                   rm.room_number AS room_number,
                   CAST(NULL AS TEXT) AS amount
            FROM allocations a
            JOIN residents r ON r.id = a.resident_id
            JOIN rooms rm ON rm.id = a.room_id
            WHERE a.hostel_id = ?1
            ORDER BY at DESC
            LIMIT ?2
            "#,
        )
        .bind(hostel_id)
        .bind(limit)
        .fetch_all(&self.inner)
        .await
        .inspect_err(
            |e| error!(error = %e, hostel_id, "database error listing check-in activity"),
        )?;

        let check_outs = sqlx::query_as::<_, ActivityItem>(
            r#"
            SELECT 'check_out' AS kind,
                   a.updated_at AS at,
                   r.full_name AS resident_name,
                   rm.room_number AS room_number,
                   CAST(NULL AS TEXT) AS amount
            FROM allocations a
            JOIN residents r ON r.id = a.resident_id
            JOIN rooms rm ON rm.id = a.room_id
            WHERE a.hostel_id = ?1
              AND a.status = 'checked_out'
            ORDER BY at DESC
            LIMIT ?2
            "#,
        )
        .bind(hostel_id)
        .bind(limit)
        .fetch_all(&self.inner)
        .await
        .inspect_err(
            |e| error!(error = %e, hostel_id, "database error listing check-out activity"),
        )?;

        let payments = sqlx::query_as::<_, ActivityItem>(
            r#"
            SELECT 'payment' AS kind,
                   p.payment_date AS at,
                   r.full_name AS resident_name,
                   rm.room_number AS room_number,
                   p.amount AS amount
            FROM payments p
            JOIN residents r ON r.id = p.resident_id
            JOIN allocations a ON a.id = p.allocation_id
            JOIN rooms rm ON rm.id = a.room_id
            WHERE p.hostel_id = ?1
            ORDER BY at DESC
            LIMIT ?2
            "#,
        )
        .bind(hostel_id)
        .bind(limit)
        .fetch_all(&self.inner)
        .await
        .inspect_err(
            |e| error!(error = %e, hostel_id, "database error listing payment activity"),
        )?;

        let mut rows = Vec::with_capacity(check_ins.len() + check_outs.len() + payments.len());
        rows.extend(check_ins);
        rows.extend(check_outs);
        rows.extend(payments);
        rows.sort_by(|a, b| b.at.cmp(&a.at));
        rows.truncate(limit as usize);
        Ok(rows)
    }

    /// Sum payments recorded on a specific calendar day.
    async fn revenue_for_date(&self, hostel_id: i64, date: NaiveDate) -> anyhow::Result<Decimal> {
        let (start, end) = local_day_bounds(date)?;
        let row = sqlx::query(
            "SELECT GROUP_CONCAT(amount) AS amounts FROM payments WHERE hostel_id = ?1 AND payment_date >= ?2 AND payment_date < ?3",
        )
        .bind(hostel_id)
        .bind(start)
        .bind(end)
        .fetch_optional(&self.inner)
        .await
        .inspect_err(|e| error!(error = %e, hostel_id, %date, "database error fetching daily revenue"))?;
        match row {
            Some(row) => {
                let amounts: Option<String> = row.try_get("amounts")?;
                sum_decimal_amounts(amounts.as_deref())
            }
            None => Ok(Decimal::ZERO),
        }
    }

    /// Sum payments recorded in a calendar month.
    async fn revenue_for_month(
        &self,
        hostel_id: i64,
        reference: NaiveDate,
    ) -> anyhow::Result<Decimal> {
        let month_start = reference
            .with_day(1)
            .ok_or_else(|| anyhow::anyhow!("invalid month start: {reference}"))?;
        let next_month = if month_start.month() == 12 {
            month_start
                .with_year(month_start.year() + 1)
                .and_then(|d: NaiveDate| d.with_month(1))
        } else {
            month_start.with_month(month_start.month() + 1)
        }
        .ok_or_else(|| anyhow::anyhow!("invalid month end: {reference}"))?;
        let (end, _) = local_day_bounds(next_month)?;
        let row = sqlx::query(
            "SELECT GROUP_CONCAT(amount) AS amounts FROM payments WHERE hostel_id = ?1 AND payment_date >= ?2 AND payment_date < ?3",
        )
        .bind(hostel_id)
        .bind(local_day_bounds(month_start)?.0)
        .bind(end)
        .fetch_optional(&self.inner)
        .await
        .inspect_err(|e| error!(error = %e, hostel_id, %reference, "database error fetching monthly revenue"))?;
        match row {
            Some(row) => {
                let amounts: Option<String> = row.try_get("amounts")?;
                sum_decimal_amounts(amounts.as_deref())
            }
            None => Ok(Decimal::ZERO),
        }
    }

    /// Sum positive balances across every allocation in the hostel, open or
    /// closed, matching [`DbPool::get_resident_detail`] and
    /// [`DbPool::list_residents`]: closing an allocation does not write off
    /// what it still owes.
    async fn outstanding_balance_total(&self, hostel_id: i64) -> anyhow::Result<Decimal> {
        let rows = sqlx::query(
            r#"
            SELECT
                a.rate_agreed,
                GROUP_CONCAT(p.amount) AS payment_amounts
            FROM allocations a
            LEFT JOIN payments p ON p.allocation_id = a.id
            WHERE a.hostel_id = ?1
            GROUP BY a.id
            "#,
        )
        .bind(hostel_id)
        .fetch_all(&self.inner)
        .await
        .inspect_err(
            |e| error!(error = %e, hostel_id, "database error fetching outstanding balances"),
        )?;

        let mut total = Decimal::ZERO;
        for row in rows {
            let rate_agreed: String = row.try_get("rate_agreed")?;
            let payment_amounts: Option<String> = row.try_get("payment_amounts")?;
            let paid = sum_decimal_amounts(payment_amounts.as_deref())?;
            let balance = allocation_balance(&rate_agreed, paid)?;
            if balance > Decimal::ZERO {
                total += balance;
            }
        }
        Ok(total)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::*;

    #[tokio::test]
    async fn list_recent_activity_includes_checkin_payment_and_checkout() {
        let pool = test_pool().await;
        let hostel = seed_hostel(&pool).await;
        let room_id = seed_room(&pool, hostel.id, "101", "single").await;
        let resident_id = seed_resident(&pool, "Liam").await;
        let allocation_id = seed_active_allocation(&pool, hostel.id, resident_id, room_id).await;

        pool.record_payment_and_receipt(NewPayment {
            hostel_id: hostel.id,
            allocation_id,
            resident_id,
            amount: "10.00".to_string(),
            payment_method: "cash".to_string(),
            period_start: None,
            period_end: None,
            notes: None,
        })
        .await
        .expect("payment");

        pool.check_out(hostel.id, allocation_id)
            .await
            .expect("checkout");

        let activity = pool
            .list_recent_activity(hostel.id, 10)
            .await
            .expect("activity");
        let kinds: Vec<_> = activity.iter().map(|a| a.kind.as_str()).collect();
        assert!(kinds.contains(&"check_in"));
        assert!(kinds.contains(&"payment"));
        assert!(kinds.contains(&"check_out"));
    }

    /// The activity feed must clamp an excessive limit to 50 rows.
    #[tokio::test]
    async fn list_recent_activity_clamps_limit_to_50() {
        let pool = test_pool().await;
        let hostel = seed_hostel(&pool).await;
        let room_id = seed_room(&pool, hostel.id, "101", "single").await;
        let resident_id = seed_resident(&pool, "Limit").await;
        seed_active_allocation(&pool, hostel.id, resident_id, room_id).await;

        let activity = pool
            .list_recent_activity(hostel.id, 1000)
            .await
            .expect("activity");
        assert!(
            activity.len() <= 50,
            "limit should be clamped to 50, got {}",
            activity.len()
        );
    }

    /// An empty hostel must report zero counts and zero revenue/balance.
    #[tokio::test]
    async fn get_dashboard_stats_zero_for_empty_hostel() {
        let pool = test_pool().await;
        let hostel = seed_hostel(&pool).await;
        let stats = pool
            .get_dashboard_stats(hostel.id, today())
            .await
            .expect("stats");
        assert_eq!(stats.total_rooms, 0);
        assert_eq!(stats.occupied, 0);
        assert_eq!(stats.vacant, 0);
        assert_eq!(stats.maintenance, 0);
        assert_eq!(stats.revenue_today, "0");
        assert_eq!(stats.revenue_month, "0");
        assert_eq!(stats.outstanding_total, "0");
    }

    /// A payment written through the normal insert path (using the
    /// `payment_date` column default, not a hardcoded fixture) must be counted
    /// in both today's and this month's revenue. This guards against the
    /// `payment_date` default and `local_day_bounds`'s bind format silently
    /// drifting apart again (see the C3 finding in the 2026-09-05 audit).
    #[tokio::test]
    async fn revenue_includes_payment_written_via_column_default() {
        let pool = test_pool().await;
        let hostel = seed_hostel(&pool).await;
        let room_id = seed_room(&pool, hostel.id, "101", "single").await;
        let resident_id = seed_resident(&pool, "Default").await;
        let allocation_id = seed_active_allocation(&pool, hostel.id, resident_id, room_id).await;

        pool.record_payment_and_receipt(NewPayment {
            hostel_id: hostel.id,
            allocation_id,
            resident_id,
            amount: "42.00".to_string(),
            payment_method: "cash".to_string(),
            period_start: None,
            period_end: None,
            notes: None,
        })
        .await
        .expect("record payment");

        let stats = pool
            .get_dashboard_stats(hostel.id, today())
            .await
            .expect("stats");
        assert_eq!(stats.revenue_today, "42.00");
        assert_eq!(stats.revenue_month, "42.00");
    }

    /// Over-payments create a negative balance for a resident, but the
    /// dashboard's outstanding total must only sum positive balances.
    #[tokio::test]
    async fn get_dashboard_stats_outstanding_ignores_negative_balances() {
        let pool = test_pool().await;
        let hostel = seed_hostel(&pool).await;
        let room_id = seed_room(&pool, hostel.id, "101", "single").await;
        let resident_id = seed_resident(&pool, "Overpaid").await;
        let allocation_id = seed_active_allocation(&pool, hostel.id, resident_id, room_id).await;

        pool.record_payment_and_receipt(NewPayment {
            hostel_id: hostel.id,
            allocation_id,
            resident_id,
            amount: "200.00".to_string(),
            payment_method: "cash".to_string(),
            period_start: None,
            period_end: None,
            notes: None,
        })
        .await
        .expect("payment");

        let stats = pool
            .get_dashboard_stats(hostel.id, today())
            .await
            .expect("stats");
        assert_eq!(stats.outstanding_total, "0");
    }
}
