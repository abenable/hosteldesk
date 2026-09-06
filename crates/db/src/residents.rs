use crate::model::*;
use crate::support::{allocation_balance, sum_decimal_amounts};
use crate::DbPool;
use rust_decimal::Decimal;
use sqlx::Row;
use std::str::FromStr;
use tracing::{debug, error, instrument};

impl DbPool {
    /// List every resident together with their latest allocation in the hostel.
    ///
    /// Residents with no allocation in this hostel show `status = "never_allocated"`
    /// and a balance of `"0"`. Active residents have their balance computed as
    /// `rate_agreed * periods_elapsed - total_paid`.
    #[instrument(skip(self))]
    pub async fn list_residents(&self, hostel_id: i64) -> anyhow::Result<Vec<ResidentSummary>> {
        debug!(hostel_id, "listing residents");
        let rows = sqlx::query(
            r#"
            SELECT
                r.id AS resident_id,
                r.full_name,
                r.phone,
                r.email,
                a.id AS allocation_id,
                rm.room_number,
                a.status AS allocation_status,
                a.rate_agreed,
                GROUP_CONCAT(p.amount) AS payment_amounts
            FROM residents r
            LEFT JOIN allocations a
              ON a.resident_id = r.id
             AND a.hostel_id = ?1
            LEFT JOIN rooms rm ON rm.id = a.room_id
            LEFT JOIN payments p ON p.allocation_id = a.id
            GROUP BY r.id, a.id
            ORDER BY r.full_name, r.id, a.id
            "#,
        )
        .bind(hostel_id)
        .fetch_all(&self.inner)
        .await
        .inspect_err(|e| error!(error = %e, hostel_id, "database error listing residents"))?;

        let mut summaries: Vec<ResidentSummary> = Vec::new();
        for row in rows {
            let resident_id: i64 = row.try_get("resident_id")?;
            let allocation_status: Option<String> = row.try_get("allocation_status")?;
            let rate_agreed: Option<String> = row.try_get("rate_agreed")?;
            let payment_amounts: Option<String> = row.try_get("payment_amounts")?;
            let status = allocation_status
                .as_deref()
                .unwrap_or("never_allocated")
                .to_string();

            let balance = match rate_agreed.as_deref() {
                Some(rate) => {
                    let paid = sum_decimal_amounts(payment_amounts.as_deref())?;
                    allocation_balance(rate, paid)?
                }
                None => Decimal::ZERO,
            };

            if let Some(existing) = summaries
                .iter_mut()
                .find(|summary| summary.id == resident_id)
            {
                let total = parse_signed_decimal(&existing.balance)? + balance;
                existing.balance = total.to_string();

                let display = status == "active" || existing.status == "never_allocated";
                if display {
                    existing.allocation_id = row.try_get("allocation_id")?;
                    existing.room_number = row.try_get("room_number")?;
                    existing.status = status;
                }
                continue;
            }

            summaries.push(ResidentSummary {
                id: resident_id,
                full_name: row.try_get("full_name")?,
                phone: row.try_get("phone")?,
                email: row.try_get("email")?,
                allocation_id: row.try_get("allocation_id")?,
                room_number: row.try_get("room_number")?,
                status,
                balance: balance.to_string(),
            });
        }

        Ok(summaries)
    }

    /// Return full resident details, including all allocations and payments in
    /// the hostel and the computed net balance.
    #[instrument(skip(self))]
    pub async fn get_resident_detail(
        &self,
        hostel_id: i64,
        resident_id: i64,
    ) -> anyhow::Result<ResidentDetail> {
        debug!(hostel_id, resident_id, "fetching resident detail");

        let resident = sqlx::query_as::<_, Resident>(
            r#"
            SELECT id, full_name, phone, email, id_number, next_of_kin, next_of_kin_phone, photo_path, created_at
            FROM residents
            WHERE id = ?1
              AND EXISTS (
                  SELECT 1 FROM allocations a
                  WHERE a.resident_id = residents.id AND a.hostel_id = ?2
              )
            "#,
        )
        .bind(resident_id)
        .bind(hostel_id)
        .fetch_optional(&self.inner)
        .await
        .inspect_err(|e| error!(error = %e, resident_id, "database error fetching resident"))?
        .ok_or_else(|| anyhow::anyhow!("resident not found"))?;

        let allocations = sqlx::query_as::<_, AllocationWithRoom>(
            r#"
            SELECT
                a.id, a.hostel_id, a.resident_id, a.room_id, a.status,
                a.rate_agreed, a.term, a.academic_year, a.created_at, r.room_number
            FROM allocations a
            JOIN rooms r ON r.id = a.room_id
            WHERE a.hostel_id = ?1 AND a.resident_id = ?2
            ORDER BY a.id DESC
            "#,
        )
        .bind(hostel_id)
        .bind(resident_id)
        .fetch_all(&self.inner)
        .await
        .inspect_err(|e| error!(error = %e, resident_id, "database error fetching allocations"))?;

        let payments = sqlx::query_as::<_, Payment>(
            "SELECT id, hostel_id, allocation_id, resident_id, amount, payment_date, payment_method, period_start, period_end, notes, created_at FROM payments WHERE hostel_id = ?1 AND resident_id = ?2 ORDER BY payment_date DESC, id DESC",
        )
        .bind(hostel_id)
        .bind(resident_id)
        .fetch_all(&self.inner)
        .await
        .inspect_err(|e| error!(error = %e, resident_id, "database error fetching payments"))?;

        let mut balance = Decimal::ZERO;
        for allocation in &allocations {
            let paid: Decimal = payments
                .iter()
                .filter(|p| p.allocation_id == allocation.id)
                .map(|p| hosteldesk_core::currency::parse_money(&p.amount))
                .collect::<Result<Vec<_>, _>>()?
                .iter()
                .copied()
                .fold(Decimal::ZERO, |acc, d| acc + d);
            balance += allocation_balance(&allocation.rate_agreed, paid)?;
        }

        Ok(ResidentDetail {
            resident,
            allocations,
            payments,
            balance: balance.to_string(),
        })
    }

    /// Update a resident's contact details.
    ///
    /// Fails when the name is empty or the resident id does not exist.
    #[instrument(skip(self))]
    pub async fn update_resident(&self, update: UpdateResident) -> anyhow::Result<()> {
        let UpdateResident {
            hostel_id,
            resident_id,
            full_name,
            phone,
            email,
            id_number,
            next_of_kin,
            next_of_kin_phone,
        } = update;
        debug!(hostel_id, resident_id, "updating resident");
        if full_name.trim().is_empty() {
            anyhow::bail!("full_name is required");
        }
        let rows = sqlx::query(
            r#"
            UPDATE residents
            SET full_name = ?1, phone = ?2, email = ?3, id_number = ?4, next_of_kin = ?5, next_of_kin_phone = ?6
            WHERE id = ?7
              AND EXISTS (
                  SELECT 1 FROM allocations a
                  WHERE a.resident_id = residents.id AND a.hostel_id = ?8
              )
            "#,
        )
        .bind(&full_name)
        .bind(&phone)
        .bind(&email)
        .bind(&id_number)
        .bind(&next_of_kin)
        .bind(&next_of_kin_phone)
        .bind(resident_id)
        .bind(hostel_id)
        .execute(&self.inner)
        .await
        .inspect_err(|e| error!(error = %e, resident_id, "database error updating resident"))?
        .rows_affected();
        if rows == 0 {
            anyhow::bail!("resident not found");
        }
        Ok(())
    }
}

/// Parse a running balance total, which is signed: a resident's net balance
/// across all their allocations may legitimately be negative (they're in
/// credit). Only for that accumulator — anything that should never be
/// negative (a rate, a single payment amount) must use
/// [`hosteldesk_core::currency::parse_money`] instead, which rejects a minus
/// sign.
fn parse_signed_decimal(amount: &str) -> anyhow::Result<Decimal> {
    Decimal::from_str(amount)
        .map_err(|e| anyhow::anyhow!("invalid decimal amount '{}': {}", amount, e))
}

/// Insert a resident inside an existing transaction.
pub(crate) async fn insert_resident(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    resident: &NewResident,
) -> anyhow::Result<i64> {
    let id = sqlx::query(
        r#"
        INSERT INTO residents (full_name, phone, email, id_number, next_of_kin, next_of_kin_phone, photo_path)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
        "#,
    )
    .bind(&resident.full_name)
    .bind(&resident.phone)
    .bind(&resident.email)
    .bind(&resident.id_number)
    .bind(&resident.next_of_kin)
    .bind(&resident.next_of_kin_phone)
    .bind(&resident.photo_path)
    .execute(&mut **tx)
    .await
    .inspect_err(|e| error!(error = %e, "database error inserting resident in transaction"))?
    .last_insert_rowid();
    Ok(id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hash_password;
    use crate::test_support::*;

    #[tokio::test]
    async fn resident_detail_returns_history_and_balance() {
        let pool = test_pool().await;
        let hostel = seed_hostel(&pool).await;
        let room_id = seed_room(&pool, hostel.id, "101", "single").await;
        let resident_id = seed_resident(&pool, "Jack").await;
        let allocation_id = seed_active_allocation(&pool, hostel.id, resident_id, room_id).await;

        pool.record_payment_and_receipt(NewPayment {
            hostel_id: hostel.id,
            allocation_id,
            resident_id,
            amount: "40.00".to_string(),
            payment_method: "cash".to_string(),
            period_start: None,
            period_end: None,
            notes: None,
        })
        .await
        .expect("payment");

        let detail = pool
            .get_resident_detail(hostel.id, resident_id)
            .await
            .expect("detail");
        assert_eq!(detail.allocations.len(), 1);
        assert_eq!(detail.payments.len(), 1);
        assert_eq!(detail.balance, "60.00");

        let err = pool
            .get_resident_detail(hostel.id, resident_id + 999)
            .await
            .expect_err("missing resident");
        assert!(err.to_string().contains("resident not found"));
    }

    #[tokio::test]
    async fn update_resident_and_list_users_round_trip() {
        let pool = test_pool().await;
        let hostel = seed_hostel(&pool).await;
        let room_id = seed_room(&pool, hostel.id, "101", "single").await;
        let resident_id = seed_resident(&pool, "Mia").await;
        seed_active_allocation(&pool, hostel.id, resident_id, room_id).await;
        pool.update_resident(UpdateResident {
            hostel_id: hostel.id,
            resident_id,
            full_name: "Mia Updated".to_string(),
            phone: Some("+256".to_string()),
            email: None,
            id_number: None,
            next_of_kin: None,
            next_of_kin_phone: None,
        })
        .await
        .expect("update");

        let detail = pool
            .get_resident_detail(hostel.id, resident_id)
            .await
            .expect("detail");
        assert_eq!(detail.resident.full_name, "Mia Updated");
        assert_eq!(detail.resident.phone.as_deref(), Some("+256"));

        pool.create_user(NewUser {
            hostel_id: hostel.id,
            full_name: "Admin".to_string(),
            email: "admin@example.com".to_string(),
            password_hash: hash_password("pw").expect("hash"),
            role: "admin".to_string(),
        })
        .await
        .expect("create staff");

        let users = pool.list_users(hostel.id).await.expect("list staff");
        assert_eq!(users.len(), 1);
        assert_eq!(users[0].full_name, "Admin");
    }

    /// Residents with no allocation in the hostel must still appear, clearly
    /// marked as never allocated and carrying a zero balance.
    #[tokio::test]
    async fn list_residents_shows_never_allocated_for_resident_without_allocations() {
        let pool = test_pool().await;
        let hostel = seed_hostel(&pool).await;
        let resident_id = seed_resident(&pool, "NoStays").await;

        let rows = pool
            .list_residents(hostel.id)
            .await
            .expect("list residents");
        let resident = rows
            .iter()
            .find(|r| r.id == resident_id)
            .expect("find resident");
        assert_eq!(resident.status, "never_allocated");
        assert_eq!(resident.balance, "0");
        assert_eq!(resident.room_number, None);
        assert_eq!(resident.allocation_id, None);
    }

    /// A resident's latest allocation must be scoped to the requested hostel.
    /// In another hostel the same resident should show as never_allocated.
    #[tokio::test]
    async fn list_residents_isolated_by_hostel_id() {
        let pool = test_pool().await;
        let hostel_a = seed_hostel(&pool).await;
        let hostel_b = seed_hostel(&pool).await;
        let room_id = seed_room(&pool, hostel_a.id, "101", "single").await;
        let resident_id = seed_resident(&pool, "Scoped").await;
        seed_active_allocation(&pool, hostel_a.id, resident_id, room_id).await;

        let rows_a = pool.list_residents(hostel_a.id).await.expect("list a");
        assert!(rows_a
            .iter()
            .any(|r| r.id == resident_id && r.status == "active"));

        let rows_b = pool.list_residents(hostel_b.id).await.expect("list b");
        let in_b = rows_b
            .iter()
            .find(|r| r.id == resident_id)
            .expect("row in hostel b");
        assert_eq!(in_b.status, "never_allocated");
    }

    /// Resident names cannot be blanked out.
    #[tokio::test]
    async fn update_resident_rejects_empty_name() {
        let pool = test_pool().await;
        let hostel = seed_hostel(&pool).await;
        let resident_id = seed_resident(&pool, "Name").await;
        let err = pool
            .update_resident(UpdateResident {
                hostel_id: hostel.id,
                resident_id,
                full_name: "   ".to_string(),
                phone: None,
                email: None,
                id_number: None,
                next_of_kin: None,
                next_of_kin_phone: None,
            })
            .await
            .expect_err("empty name");
        assert!(err.to_string().contains("full_name"));
    }

    /// Updating an unknown resident must fail rather than silently no-op.
    #[tokio::test]
    async fn update_resident_rejects_unknown_resident() {
        let pool = test_pool().await;
        let hostel = seed_hostel(&pool).await;
        let err = pool
            .update_resident(UpdateResident {
                hostel_id: hostel.id,
                resident_id: 99999,
                full_name: "New Name".to_string(),
                phone: None,
                email: None,
                id_number: None,
                next_of_kin: None,
                next_of_kin_phone: None,
            })
            .await
            .expect_err("unknown resident");
        assert!(err.to_string().contains("not found"));
    }

    #[tokio::test]
    async fn resident_detail_and_update_are_isolated_by_hostel() {
        let pool = test_pool().await;
        let hostel_a = seed_hostel(&pool).await;
        let hostel_b = seed_hostel(&pool).await;
        let room_a = seed_room(&pool, hostel_a.id, "A101", "single").await;
        let resident_id = seed_resident(&pool, "Cross Hostel Resident").await;
        seed_active_allocation(&pool, hostel_a.id, resident_id, room_a).await;

        let detail_err = pool
            .get_resident_detail(hostel_b.id, resident_id)
            .await
            .expect_err("resident without an allocation in hostel B must be hidden");
        assert!(detail_err.to_string().contains("resident not found"));

        let update_err = pool
            .update_resident(UpdateResident {
                hostel_id: hostel_b.id,
                resident_id,
                full_name: "Changed Name".to_string(),
                phone: None,
                email: None,
                id_number: None,
                next_of_kin: None,
                next_of_kin_phone: None,
            })
            .await
            .expect_err("resident update must be scoped to hostel B");
        assert!(update_err.to_string().contains("resident not found"));

        let name: String = sqlx::query_scalar("SELECT full_name FROM residents WHERE id = ?1")
            .bind(resident_id)
            .fetch_one(&pool.inner)
            .await
            .expect("fetch resident name");
        assert_eq!(name, "Cross Hostel Resident");
    }
}
