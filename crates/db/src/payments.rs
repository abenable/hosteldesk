use crate::model::*;
use crate::support::local_day_bounds;
use crate::DbPool;
use tracing::{debug, error, info, instrument};

impl DbPool {
    /// Record a payment and atomically create the next sequential receipt for it.
    ///
    /// Returns the new `payment_id` and the generated `receipt_number` as a string.
    /// The receipt number is allocated per hostel using `BEGIN IMMEDIATE` so that
    /// concurrent writes are serialized and the sequence stays gapless under
    /// successful commits.
    #[instrument(skip(self))]
    pub async fn record_payment_and_receipt(
        &self,
        payment: NewPayment,
    ) -> anyhow::Result<(i64, String)> {
        debug!(
            allocation_id = payment.allocation_id,
            "recording payment with receipt"
        );
        let mut tx =
            self.inner.begin_with("BEGIN IMMEDIATE").await.inspect_err(
                |e| error!(error = %e, "database error beginning receipt transaction"),
            )?;

        let next_number: i64 = sqlx::query_scalar(
            "SELECT COALESCE(MAX(CAST(receipt_number AS INTEGER)), 0) + 1 FROM receipts WHERE hostel_id = ?1"
        )
        .bind(payment.hostel_id)
        .fetch_one(&mut *tx)
        .await
        .inspect_err(|e| error!(error = %e, hostel_id = payment.hostel_id, "database error allocating receipt number"))?;

        let receipt_number = next_number.to_string();

        let payment_id = sqlx::query(
            r#"
            INSERT INTO payments
                (hostel_id, allocation_id, resident_id, amount, payment_method, period_start, period_end, notes)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            "#,
        )
        .bind(payment.hostel_id)
        .bind(payment.allocation_id)
        .bind(payment.resident_id)
        .bind(payment.amount)
        .bind(&payment.payment_method)
        .bind(payment.period_start)
        .bind(payment.period_end)
        .bind(payment.notes)
        .execute(&mut *tx)
        .await
        .inspect_err(|e| error!(error = %e, "database error inserting payment in receipt transaction"))?
        .last_insert_rowid();

        sqlx::query(
            "INSERT INTO receipts (hostel_id, payment_id, receipt_number) VALUES (?1, ?2, ?3)",
        )
        .bind(payment.hostel_id)
        .bind(payment_id)
        .bind(&receipt_number)
        .execute(&mut *tx)
        .await
        .inspect_err(|e| error!(error = %e, payment_id, "database error inserting receipt"))?;

        tx.commit().await.inspect_err(
            |e| error!(error = %e, payment_id, "database error committing receipt transaction"),
        )?;

        info!(payment_id, %receipt_number, "payment recorded with receipt");
        Ok((payment_id, receipt_number))
    }

    /// Update `receipts.pdf_path` for a given hostel and receipt number.
    ///
    /// Returns `Ok(())` whether a row was updated or not. A missing row is
    /// logged at debug level to support legacy receipts without failing the
    /// caller.
    #[instrument(skip(self))]
    pub async fn update_receipt_pdf_path(
        &self,
        hostel_id: i64,
        receipt_number: &str,
        pdf_path: &str,
    ) -> anyhow::Result<()> {
        debug!(hostel_id, %receipt_number, %pdf_path, "updating receipt pdf_path");

        let rows = sqlx::query(
            "UPDATE receipts SET pdf_path = ?1 WHERE hostel_id = ?2 AND receipt_number = ?3",
        )
        .bind(pdf_path)
        .bind(hostel_id)
        .bind(receipt_number)
        .execute(&self.inner)
        .await
        .inspect_err(|e| error!(error = %e, hostel_id, %receipt_number, "database error updating receipt pdf_path"))?
        .rows_affected();

        if rows == 0 {
            debug!(hostel_id, %receipt_number, "no receipt row found for pdf_path update");
        } else {
            info!(hostel_id, %receipt_number, "receipt pdf_path updated");
        }

        Ok(())
    }

    /// Return the payments ledger for a hostel, optionally filtered.
    #[instrument(skip(self))]
    pub async fn list_payments(
        &self,
        filter: ListPaymentsFilter,
    ) -> anyhow::Result<Vec<PaymentRow>> {
        debug!(hostel_id = filter.hostel_id, "listing payments");
        let from = filter
            .from
            .map(local_day_bounds)
            .transpose()?
            .map(|(start, _)| start);
        let to = filter
            .to
            .map(local_day_bounds)
            .transpose()?
            .map(|(_, end)| end);
        let rows = sqlx::query_as::<_, PaymentRow>(
            r#"
            SELECT
                p.id, p.resident_id, r.full_name AS resident_name,
                p.allocation_id, rm.room_number, p.amount, p.payment_method,
                p.payment_date, rc.receipt_number, p.notes
            FROM payments p
            JOIN residents r ON r.id = p.resident_id
            JOIN allocations a ON a.id = p.allocation_id
            JOIN rooms rm ON rm.id = a.room_id
            LEFT JOIN receipts rc ON rc.payment_id = p.id
            WHERE p.hostel_id = ?1
              AND p.payment_date >= COALESCE(?2, '0001-01-01T00:00:00Z')
              AND p.payment_date < COALESCE(?3, '9999-12-31T23:59:59Z')
              AND (?4 IS NULL OR p.resident_id = ?4)
              AND (?5 IS NULL OR p.payment_method = ?5)
            ORDER BY p.payment_date DESC, p.id DESC
            LIMIT 500 -- ponytail: hard ceiling; add pagination if users exceed 500 rows
            "#,
        )
        .bind(filter.hostel_id)
        .bind(from)
        .bind(to)
        .bind(filter.resident_id)
        .bind(filter.method)
        .fetch_all(&self.inner)
        .await
        .inspect_err(
            |e| error!(error = %e, hostel_id = filter.hostel_id, "database error listing payments"),
        )?;
        Ok(rows)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::*;
    use chrono::NaiveDate;
    use sqlx::Row;

    #[tokio::test]
    async fn concurrent_record_payment_and_receipt_are_sequential_and_distinct() {
        let dir = tempfile::tempdir().expect("create temp dir");
        let db_path = dir.path().join("hosteldesk.db");
        let url = format!("sqlite:{}", db_path.display());
        let pool = DbPool::open(&url).await.expect("open pool");

        let hostel = pool
            .create_hostel(NewHostel {
                name: "H".to_string(),
                address: None,
                phone: None,
                currency: None,
                branding_logo_path: None,
            })
            .await
            .expect("create hostel");

        let room_id = pool
            .add_room(NewRoom {
                hostel_id: hostel.id,
                room_number: "101".to_string(),
                room_type: "single".to_string(),
                capacity: 1,
                floor: None,
                semester_rate: "10".to_string(),
                recess_rate: "10".to_string(),
            })
            .await
            .expect("add room");

        let resident_id = seed_resident(&pool, "Resident").await;

        let allocation_id = pool
            .check_in(NewCheckIn {
                hostel_id: hostel.id,
                resident_id: Some(resident_id),
                full_name: None,
                phone: None,
                email: None,
                id_number: None,
                next_of_kin: None,
                next_of_kin_phone: None,
                photo_path: None,
                room_id,
                rate_agreed: "10.00".to_string(),
                term: "semester_2".to_string(),
                academic_year: "2025/2026".to_string(),
            })
            .await
            .expect("create allocation");

        let payment = NewPayment {
            hostel_id: hostel.id,
            allocation_id,
            resident_id,
            amount: "5.00".to_string(),
            payment_method: "cash".to_string(),
            period_start: None,
            period_end: None,
            notes: None,
        };

        let (result_a, result_b) = tokio::join!(
            pool.record_payment_and_receipt(payment.clone()),
            pool.record_payment_and_receipt(payment.clone()),
        );

        let (id_a, number_a) = result_a.expect("first payment");
        let (id_b, number_b) = result_b.expect("second payment");

        assert_ne!(id_a, id_b, "payment ids should differ");
        assert_ne!(number_a, number_b, "receipt numbers should differ");
        assert!(
            number_b.parse::<i64>().expect("numeric receipt")
                > number_a.parse::<i64>().expect("numeric receipt"),
            "receipt numbers should be sequential"
        );
    }

    #[tokio::test]
    async fn update_receipt_pdf_path_persists_and_ignores_missing() {
        let pool = test_pool().await;
        let hostel = seed_hostel(&pool).await;
        let room_id = seed_room(&pool, hostel.id, "101", "single").await;
        let resident_id = seed_resident(&pool, "Receipt").await;
        let allocation_id = pool
            .check_in(NewCheckIn {
                hostel_id: hostel.id,
                resident_id: Some(resident_id),
                full_name: None,
                phone: None,
                email: None,
                id_number: None,
                next_of_kin: None,
                next_of_kin_phone: None,
                photo_path: None,
                room_id,
                rate_agreed: "100.00".to_string(),
                term: "semester_1".to_string(),
                academic_year: "2026/2027".to_string(),
            })
            .await
            .expect("create allocation");

        let (_, receipt_number) = pool
            .record_payment_and_receipt(NewPayment {
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
            .expect("record payment");

        pool.update_receipt_pdf_path(hostel.id, &receipt_number, "/tmp/receipt.pdf")
            .await
            .expect("update pdf_path");

        let stored: Option<String> = sqlx::query_scalar(
            "SELECT pdf_path FROM receipts WHERE hostel_id = ?1 AND receipt_number = ?2",
        )
        .bind(hostel.id)
        .bind(&receipt_number)
        .fetch_one(&pool.inner)
        .await
        .expect("fetch pdf_path");
        assert_eq!(stored.as_deref(), Some("/tmp/receipt.pdf"));

        // A missing receipt should return Ok without affecting any row.
        pool.update_receipt_pdf_path(hostel.id, "99999", "/tmp/missing.pdf")
            .await
            .expect("missing receipt should not error");
    }

    #[tokio::test]
    async fn payments_query_plan_uses_hostel_date_index() {
        let pool = test_pool().await;
        // Matches list_payments's actual predicate shape (hostel_id + a date
        // range), not a bare `WHERE hostel_id = ?` — both idx_payments_hostel_date
        // and idx_payments_hostel_resident satisfy the latter equally, so it
        // doesn't pin down which one SQLite should pick here.
        let rows = sqlx::query(
            "EXPLAIN QUERY PLAN SELECT * FROM payments WHERE hostel_id = ?1 AND payment_date >= ?2 AND payment_date < ?3",
        )
        .bind(1i64)
        .bind("2026-01-01T00:00:00Z")
        .bind("2026-02-01T00:00:00Z")
        .fetch_all(&pool.inner)
        .await
        .expect("query plan");

        let details: Vec<String> = rows
            .iter()
            .map(|r| r.try_get::<String, _>("detail").expect("detail column"))
            .collect();
        assert!(
            details
                .iter()
                .any(|d| d.contains("idx_payments_hostel_date")),
            "payments date-range lookup should use idx_payments_hostel_date, got: {details:?}"
        );
    }

    /// P3: get_resident_detail's per-resident payment lookup must be a
    /// covering index search, not a hostel-wide scan filtered row-by-row.
    #[tokio::test]
    async fn payments_query_plan_uses_hostel_resident_index() {
        let pool = test_pool().await;
        let rows = sqlx::query(
            "EXPLAIN QUERY PLAN SELECT * FROM payments WHERE hostel_id = ?1 AND resident_id = ?2",
        )
        .bind(1i64)
        .bind(1i64)
        .fetch_all(&pool.inner)
        .await
        .expect("query plan");

        let details: Vec<String> = rows
            .iter()
            .map(|r| r.try_get::<String, _>("detail").expect("detail column"))
            .collect();
        assert!(
            details
                .iter()
                .any(|d| d.contains("idx_payments_hostel_resident")),
            "resident-scoped payments lookup should use idx_payments_hostel_resident, got: {details:?}"
        );
    }

    /// P4: list_payments must stay sargable when the date filters are absent
    /// (the common case — Invoices.tsx doesn't wire a filter UI yet). The old
    /// `(?N IS NULL OR ...)` wrapper defeated the index range scan.
    #[tokio::test]
    async fn payments_query_plan_stays_sargable_with_no_date_filter() {
        let pool = test_pool().await;
        let rows = sqlx::query(
            r#"
            EXPLAIN QUERY PLAN SELECT * FROM payments p
            WHERE p.hostel_id = ?1
              AND p.payment_date >= COALESCE(?2, '0001-01-01T00:00:00Z')
              AND p.payment_date < COALESCE(?3, '9999-12-31T23:59:59Z')
            "#,
        )
        .bind(1i64)
        .bind(None::<String>)
        .bind(None::<String>)
        .fetch_all(&pool.inner)
        .await
        .expect("query plan");

        let details: Vec<String> = rows
            .iter()
            .map(|r| r.try_get::<String, _>("detail").expect("detail column"))
            .collect();
        assert!(
            details
                .iter()
                .any(|d| { d.contains("idx_payments_hostel_date") && d.contains("payment_date") }),
            "unfiltered list_payments should still range-scan on payment_date, got: {details:?}"
        );
    }

    /// Date range filters on the payments ledger must use inclusive from/to
    /// calendar days and exclude payments outside the range.
    #[tokio::test]
    async fn list_payments_filters_by_date_range() {
        let pool = test_pool().await;
        let hostel = seed_hostel(&pool).await;
        let room_id = seed_room(&pool, hostel.id, "101", "single").await;
        let resident_id = seed_resident(&pool, "DateFilter").await;
        let allocation_id = seed_active_allocation(&pool, hostel.id, resident_id, room_id).await;

        for (amount, date) in [
            ("10.00", "2026-07-09T10:00:00Z"),
            ("20.00", "2026-07-10T10:00:00Z"),
            ("30.00", "2026-07-11T10:00:00Z"),
        ] {
            sqlx::query(
                "INSERT INTO payments (hostel_id, allocation_id, resident_id, amount, payment_date, payment_method) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            )
            .bind(hostel.id)
            .bind(allocation_id)
            .bind(resident_id)
            .bind(amount)
            .bind(date)
            .bind("cash")
            .execute(&pool.inner)
            .await
            .expect("insert payment");
        }

        let rows = pool
            .list_payments(ListPaymentsFilter {
                hostel_id: hostel.id,
                from: Some(NaiveDate::from_ymd_opt(2026, 7, 10).unwrap()),
                to: Some(NaiveDate::from_ymd_opt(2026, 7, 10).unwrap()),
                resident_id: None,
                method: None,
            })
            .await
            .expect("list");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].amount, "20.00");
    }

    /// The resident_id filter must restrict payments to a single resident.
    #[tokio::test]
    async fn list_payments_filters_by_resident_id() {
        let pool = test_pool().await;
        let hostel = seed_hostel(&pool).await;
        let room_a = seed_room(&pool, hostel.id, "101", "single").await;
        let room_b = seed_room(&pool, hostel.id, "102", "single").await;
        let resident_a = seed_resident(&pool, "A").await;
        let resident_b = seed_resident(&pool, "B").await;
        let alloc_a = seed_active_allocation(&pool, hostel.id, resident_a, room_a).await;
        let alloc_b = seed_active_allocation(&pool, hostel.id, resident_b, room_b).await;

        pool.record_payment_and_receipt(NewPayment {
            hostel_id: hostel.id,
            allocation_id: alloc_a,
            resident_id: resident_a,
            amount: "10.00".to_string(),
            payment_method: "cash".to_string(),
            period_start: None,
            period_end: None,
            notes: None,
        })
        .await
        .expect("payment a");
        pool.record_payment_and_receipt(NewPayment {
            hostel_id: hostel.id,
            allocation_id: alloc_b,
            resident_id: resident_b,
            amount: "20.00".to_string(),
            payment_method: "cash".to_string(),
            period_start: None,
            period_end: None,
            notes: None,
        })
        .await
        .expect("payment b");

        let rows = pool
            .list_payments(ListPaymentsFilter {
                hostel_id: hostel.id,
                from: None,
                to: None,
                resident_id: Some(resident_a),
                method: None,
            })
            .await
            .expect("list");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].resident_name, "A");
    }
}
