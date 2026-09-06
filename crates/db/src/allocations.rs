use crate::model::*;
use crate::residents::insert_resident;
use crate::support::{allocation_balance, sum_decimal_amounts};
use crate::DbPool;
use sqlx::Row;
use tracing::{debug, error, info, instrument};

impl DbPool {
    /// Check a resident into a room atomically.
    ///
    /// If `check_in.resident_id` is `Some`, it is used directly; otherwise a new
    /// resident is created from `full_name` and the other contact fields. The
    /// resident creation and room allocation share a single transaction: if the
    /// allocation cannot be created, no resident row is left behind.
    #[instrument(skip(self, check_in))]
    pub async fn check_in(&self, check_in: NewCheckIn) -> anyhow::Result<i64> {
        debug!(room_id = check_in.room_id, "checking in resident");
        let mut tx =
            self.inner.begin().await.inspect_err(
                |e| error!(error = %e, "database error starting check-in transaction"),
            )?;

        let resident_id = match check_in.resident_id {
            Some(id) => id,
            None => {
                let name = check_in.full_name.as_deref().unwrap_or("").trim();
                if name.is_empty() {
                    anyhow::bail!("full_name is required for a new resident");
                }
                insert_resident(
                    &mut tx,
                    &NewResident {
                        full_name: name.to_string(),
                        phone: check_in.phone.clone(),
                        email: check_in.email.clone(),
                        id_number: check_in.id_number.clone(),
                        next_of_kin: check_in.next_of_kin.clone(),
                        next_of_kin_phone: check_in.next_of_kin_phone.clone(),
                        photo_path: check_in.photo_path.clone(),
                    },
                )
                .await?
            }
        };

        let allocation_id = insert_allocation(
            &mut tx,
            &NewAllocation {
                hostel_id: check_in.hostel_id,
                resident_id,
                room_id: check_in.room_id,
                rate_agreed: check_in.rate_agreed,
                term: check_in.term,
                academic_year: check_in.academic_year,
            },
        )
        .await?;

        tx.commit().await.inspect_err(
            |e| error!(error = %e, allocation_id, "database error committing check-in"),
        )?;

        info!(allocation_id, resident_id, "check-in complete");
        Ok(allocation_id)
    }

    /// Check out an active allocation and free its room when appropriate.
    ///
    /// Fails when the allocation is not active or the database transaction
    /// cannot complete.
    #[instrument(skip(self))]
    pub async fn check_out(&self, hostel_id: i64, allocation_id: i64) -> anyhow::Result<()> {
        debug!(hostel_id, allocation_id, "checking out allocation");
        let mut tx =
            self.inner.begin().await.inspect_err(
                |e| error!(error = %e, "database error starting checkout transaction"),
            )?;

        let row = sqlx::query(
            "SELECT room_id FROM allocations WHERE id = ?1 AND hostel_id = ?2 AND status = 'active'",
        )
        .bind(allocation_id)
        .bind(hostel_id)
        .fetch_optional(&mut *tx)
        .await
        .inspect_err(|e| error!(error = %e, allocation_id, "database error locking allocation"))?;

        let Some(row) = row else {
            anyhow::bail!("allocation is not active");
        };
        let room_id: i64 = row.try_get("room_id")?;

        sqlx::query(
            "UPDATE allocations SET status = 'checked_out', updated_at = CURRENT_TIMESTAMP WHERE id = ?1 AND hostel_id = ?2",
        )
        .bind(allocation_id)
        .bind(hostel_id)
        .execute(&mut *tx)
        .await
        .inspect_err(|e| error!(error = %e, allocation_id, "database error updating allocation"))?;

        sqlx::query(
            r#"
            UPDATE rooms
            SET status = 'vacant'
            WHERE id = ?1
              AND hostel_id = ?2
              AND status = 'occupied'
              AND NOT EXISTS (
                  SELECT 1 FROM allocations
                  WHERE room_id = ?1 AND hostel_id = ?2 AND status = 'active'
              )
            "#,
        )
        .bind(room_id)
        .bind(hostel_id)
        .execute(&mut *tx)
        .await
        .inspect_err(|e| error!(error = %e, room_id, "database error updating room status"))?;

        tx.commit().await.inspect_err(
            |e| error!(error = %e, allocation_id, "database error committing checkout"),
        )?;

        info!(allocation_id, "allocation checked out");
        Ok(())
    }

    /// Return active allocations for the payment-form resident picker, with
    /// accrued balance folded in Rust using exact decimal arithmetic.
    #[instrument(skip(self))]
    pub async fn list_active_allocations(
        &self,
        hostel_id: i64,
    ) -> anyhow::Result<Vec<ActiveAllocation>> {
        debug!(hostel_id, "listing active allocations");
        let rows = sqlx::query(
            r#"
            SELECT
                a.id AS allocation_id, a.resident_id,
                r.full_name AS resident_name, rm.room_number,
                a.rate_agreed, a.term, a.academic_year,
                GROUP_CONCAT(p.amount) AS payment_amounts
            FROM allocations a
            JOIN residents r ON r.id = a.resident_id
            JOIN rooms rm ON rm.id = a.room_id
            LEFT JOIN payments p ON p.allocation_id = a.id
            WHERE a.hostel_id = ?1 AND a.status = 'active'
            GROUP BY a.id
            ORDER BY r.full_name
            "#,
        )
        .bind(hostel_id)
        .fetch_all(&self.inner)
        .await
        .inspect_err(
            |e| error!(error = %e, hostel_id, "database error listing active allocations"),
        )?;

        let mut results = Vec::with_capacity(rows.len());
        for row in rows {
            let allocation_id: i64 = row.try_get("allocation_id")?;
            let resident_id: i64 = row.try_get("resident_id")?;
            let resident_name: String = row.try_get("resident_name")?;
            let room_number: String = row.try_get("room_number")?;
            let rate_agreed: String = row.try_get("rate_agreed")?;
            let term: String = row.try_get("term")?;
            let academic_year: String = row.try_get("academic_year")?;
            let payment_amounts: Option<String> = row.try_get("payment_amounts")?;

            let total_paid = sum_decimal_amounts(payment_amounts.as_deref())?;
            let balance = allocation_balance(&rate_agreed, total_paid)?;

            results.push(ActiveAllocation {
                allocation_id,
                resident_id,
                resident_name,
                room_number,
                rate_agreed,
                term,
                academic_year,
                total_paid: total_paid.to_string(),
                balance: balance.to_string(),
            });
        }

        Ok(results)
    }
}

/// Verify a room can be allocated and, if it is vacant, mark it occupied.
///
/// Fails when the room does not belong to the hostel, is under maintenance,
/// or already has active allocations equal to its capacity. Called by
/// [`insert_allocation`] so every allocation path applies the same guards.
async fn prepare_room_for_allocation(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    hostel_id: i64,
    room_id: i64,
) -> anyhow::Result<()> {
    let row = sqlx::query("SELECT status, capacity FROM rooms WHERE id = ?1 AND hostel_id = ?2")
        .bind(room_id)
        .bind(hostel_id)
        .fetch_optional(&mut **tx)
        .await
        .inspect_err(
            |e| error!(error = %e, room_id, "database error fetching room for allocation"),
        )?;

    let Some(row) = row else {
        anyhow::bail!("room not found or does not belong to the hostel");
    };

    let status: String = row.try_get("status")?;
    let capacity: i32 = row.try_get("capacity")?;

    if status == "maintenance" {
        anyhow::bail!("room is under maintenance");
    }
    if !matches!(status.as_str(), "vacant" | "occupied") {
        anyhow::bail!("room cannot be allocated");
    }

    let active_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM allocations WHERE room_id = ?1 AND hostel_id = ?2 AND status = 'active'",
    )
    .bind(room_id)
    .bind(hostel_id)
    .fetch_one(&mut **tx)
    .await
    .inspect_err(|e| error!(error = %e, room_id, "database error counting active allocations"))?;

    if active_count >= i64::from(capacity) {
        anyhow::bail!("room has no available beds");
    }

    if status == "vacant" {
        let rows = sqlx::query(
            "UPDATE rooms SET status = 'occupied' WHERE id = ?1 AND hostel_id = ?2 AND status = 'vacant'",
        )
        .bind(room_id)
        .bind(hostel_id)
        .execute(&mut **tx)
        .await
        .inspect_err(|e| error!(error = %e, room_id, "database error updating room status"))?
        .rows_affected();

        if rows == 0 {
            anyhow::bail!("room is no longer available");
        }
    }

    Ok(())
}

/// Insert an allocation inside an existing transaction after verifying the room
/// is allocatable and updating its status.
pub(crate) async fn insert_allocation(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    allocation: &NewAllocation,
) -> anyhow::Result<i64> {
    if !matches!(
        allocation.term.as_str(),
        "semester_1" | "semester_2" | "recess"
    ) {
        anyhow::bail!("term must be one of: semester_1, semester_2, recess");
    }
    prepare_room_for_allocation(tx, allocation.hostel_id, allocation.room_id).await?;

    let id = sqlx::query(
        r#"
        INSERT INTO allocations
            (hostel_id, resident_id, room_id, status, rate_agreed, term, academic_year)
        VALUES (?1, ?2, ?3, 'active', ?4, ?5, ?6)
        "#,
    )
    .bind(allocation.hostel_id)
    .bind(allocation.resident_id)
    .bind(allocation.room_id)
    .bind(&allocation.rate_agreed)
    .bind(&allocation.term)
    .bind(&allocation.academic_year)
    .execute(&mut **tx)
    .await
    .inspect_err(|e| error!(error = %e, "database error inserting allocation"))?
    .last_insert_rowid();

    Ok(id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::*;

    /// C2: closing an allocation with an unpaid balance must not write off
    /// that debt — it keeps counting toward the hostel's outstanding total.
    #[tokio::test]
    async fn closed_allocation_debt_still_counts_toward_outstanding_total() {
        let pool = test_pool().await;
        let hostel = seed_hostel(&pool).await;
        let room_id = seed_room(&pool, hostel.id, "101", "single").await;
        let resident_id = seed_resident(&pool, "Left Owing").await;
        let allocation_id = seed_active_allocation(&pool, hostel.id, resident_id, room_id).await;

        pool.check_out(hostel.id, allocation_id)
            .await
            .expect("check out");

        let stats = pool
            .get_dashboard_stats(hostel.id, today())
            .await
            .expect("stats");
        assert_eq!(
            stats.outstanding_total, "100",
            "an unpaid, closed allocation must still be outstanding"
        );
    }

    #[tokio::test]
    async fn list_active_allocations_returns_paid_and_balance() {
        let pool = test_pool().await;
        let hostel = seed_hostel(&pool).await;
        let room_id = seed_room(&pool, hostel.id, "101", "single").await;
        let resident_id = seed_resident(&pool, "Kim").await;
        let allocation_id = seed_active_allocation(&pool, hostel.id, resident_id, room_id).await;

        pool.record_payment_and_receipt(NewPayment {
            hostel_id: hostel.id,
            allocation_id,
            resident_id,
            amount: "25.00".to_string(),
            payment_method: "cash".to_string(),
            period_start: None,
            period_end: None,
            notes: None,
        })
        .await
        .expect("payment");

        let active = pool
            .list_active_allocations(hostel.id)
            .await
            .expect("active allocations");
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].total_paid, "25.00");
        assert_eq!(active[0].balance, "75.00");
    }

    /// Checking out an allocation frees its room.
    #[tokio::test]
    async fn checkout_frees_the_room() {
        let pool = test_pool().await;
        let hostel = seed_hostel(&pool).await;
        let room_id = seed_room(&pool, hostel.id, "101", "single").await;
        let resident_id = seed_resident(&pool, "SameDay").await;
        let allocation_id = seed_active_allocation(&pool, hostel.id, resident_id, room_id).await;

        pool.check_out(hostel.id, allocation_id)
            .await
            .expect("checkout");
        let room = pool
            .get_rooms(hostel.id)
            .await
            .expect("rooms")
            .into_iter()
            .find(|r| r.id == room_id)
            .expect("room");
        assert_eq!(room.status, "vacant");
    }

    /// Checking out a non-existent allocation is treated the same as an
    /// inactive allocation: a clear "not active" error.
    #[tokio::test]
    async fn checkout_unknown_allocation_is_not_active() {
        let pool = test_pool().await;
        let hostel = seed_hostel(&pool).await;
        let err = pool
            .check_out(hostel.id, 99999)
            .await
            .expect_err("unknown allocation");
        assert!(err.to_string().contains("not active"));
    }

    /// The resident picker must return an empty list when no one is checked in.
    #[tokio::test]
    async fn list_active_allocations_empty_for_no_active_stays() {
        let pool = test_pool().await;
        let hostel = seed_hostel(&pool).await;
        let active = pool
            .list_active_allocations(hostel.id)
            .await
            .expect("list active");
        assert!(active.is_empty());
    }
}
