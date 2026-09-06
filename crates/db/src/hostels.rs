use crate::model::*;
use crate::DbPool;
use sqlx::Row;
use tracing::{debug, error, info, instrument};

impl DbPool {
    /// Complete first-run onboarding by creating one hostel and one admin.
    ///
    /// The check and both inserts share a `BEGIN IMMEDIATE` transaction, so the
    /// operation is one-time even if two callers race. The transaction guard
    /// rolls back automatically if the future is cancelled.
    #[instrument(skip(self, input))]
    pub async fn complete_onboarding(&self, input: CompleteOnboarding) -> anyhow::Result<User> {
        if input.hostel.name.trim().is_empty() {
            anyhow::bail!("hostel name is required");
        }
        if input.admin_name.trim().is_empty() {
            anyhow::bail!("admin name is required");
        }
        if input.email.trim().is_empty() {
            anyhow::bail!("email is required");
        }

        let mut tx = self.inner.begin_with("BEGIN IMMEDIATE").await.inspect_err(
            |e| error!(error = %e, "database error starting onboarding transaction"),
        )?;

        let hostel_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM hostels")
            .fetch_one(&mut *tx)
            .await
            .inspect_err(|e| error!(error = %e, "database error counting hostels"))?;
        if hostel_count != 0 {
            anyhow::bail!("onboarding is already complete");
        }

        let hostel_id = sqlx::query(
            r#"
            INSERT INTO hostels (name, address, phone, currency, branding_logo_path)
            VALUES (?1, ?2, ?3, ?4, ?5)
            "#,
        )
        .bind(input.hostel.name.trim())
        .bind(input.hostel.address)
        .bind(input.hostel.phone)
        .bind(input.hostel.currency.as_deref().unwrap_or("UGX"))
        .bind(input.hostel.branding_logo_path)
        .execute(&mut *tx)
        .await
        .inspect_err(|e| error!(error = %e, "database error inserting onboarding hostel"))?
        .last_insert_rowid();

        let user_id = sqlx::query(
            r#"
            INSERT INTO users (hostel_id, full_name, email, password_hash, role)
            VALUES (?1, ?2, ?3, ?4, 'admin')
            "#,
        )
        .bind(hostel_id)
        .bind(input.admin_name.trim())
        .bind(input.email.trim())
        .bind(input.password_hash)
        .execute(&mut *tx)
        .await
        .inspect_err(|e| error!(error = %e, "database error inserting onboarding admin"))?
        .last_insert_rowid();

        let user = sqlx::query_as::<_, User>(
            "SELECT id, hostel_id, full_name, email, role, created_at FROM users WHERE id = ?1",
        )
        .bind(user_id)
        .fetch_one(&mut *tx)
        .await
        .inspect_err(|e| error!(error = %e, user_id, "database error fetching onboarding admin"))?;

        tx.commit()
            .await
            .inspect_err(|e| error!(error = %e, "database error committing onboarding"))?;

        info!(hostel_id, user_id, "onboarding completed");
        Ok(user)
    }

    /// Insert a new hostel and return the full row (id, created_at, currency).
    ///
    /// `currency` defaults to `"UGX"` when `None`. Other fields are optional.
    #[instrument(skip(self, hostel))]
    pub async fn create_hostel(&self, hostel: NewHostel) -> anyhow::Result<Hostel> {
        debug!(name = %hostel.name, "creating hostel");
        let row = sqlx::query_as::<_, Hostel>(
            r#"
            INSERT INTO hostels (name, address, phone, currency, branding_logo_path)
            VALUES (?1, ?2, ?3, ?4, ?5)
            RETURNING id, name, address, phone, currency, branding_logo_path, created_at
            "#,
        )
        .bind(hostel.name)
        .bind(hostel.address)
        .bind(hostel.phone)
        .bind(hostel.currency.as_deref().unwrap_or("UGX"))
        .bind(hostel.branding_logo_path)
        .fetch_one(&self.inner)
        .await
        .inspect_err(|e| error!(error = %e, "database error inserting hostel"))?;

        info!(hostel_id = row.id, "hostel inserted");
        Ok(row)
    }

    /// Return the hostel profile with derived `room_count`, `admin_count`,
    /// and `onboarding_complete` flag, or `None` if the hostel doesn't exist.
    #[instrument(skip(self))]
    pub async fn get_hostel_profile(
        &self,
        hostel_id: i64,
    ) -> anyhow::Result<Option<HostelProfile>> {
        debug!(hostel_id, "fetching hostel profile");
        let row = sqlx::query_as::<_, HostelProfile>(
            r#"
            SELECT
              h.id AS id, h.name AS name, h.address AS address, h.phone AS phone,
              h.currency AS currency, h.branding_logo_path AS branding_logo_path,
              (SELECT COUNT(*) FROM rooms r WHERE r.hostel_id = h.id) AS room_count,
              (SELECT COUNT(*) FROM users s WHERE s.hostel_id = h.id AND s.role = 'admin') AS admin_count,
              0 AS onboarding_complete
            FROM hostels h
            WHERE h.id = ?1
            "#,
        )
        .bind(hostel_id)
        .fetch_optional(&self.inner)
        .await
        .inspect_err(|e| error!(error = %e, hostel_id, "database error fetching hostel profile"))?;

        match row {
            Some(mut p) => {
                p.onboarding_complete = p.room_count > 0 && p.admin_count > 0;
                Ok(Some(p))
            }
            None => Ok(None),
        }
    }

    /// Fetch a hostel by id.
    ///
    /// Returns `None` when no hostel with the supplied id exists.
    #[instrument(skip(self))]
    pub async fn get_hostel_by_id(&self, hostel_id: i64) -> anyhow::Result<Option<Hostel>> {
        debug!(hostel_id, "fetching hostel by id");
        let hostel = sqlx::query_as::<_, Hostel>(
            "SELECT id, name, address, phone, currency, branding_logo_path, created_at FROM hostels WHERE id = ?1",
        )
        .bind(hostel_id)
        .fetch_optional(&self.inner)
        .await
        .inspect_err(|e| error!(error = %e, hostel_id, "database error fetching hostel"))?;
        Ok(hostel)
    }

    /// Return whether first-run onboarding has produced a usable hostel.
    ///
    /// Onboarding is complete when exactly one hostel exists and it has at
    /// least one staff account.
    #[instrument(skip(self))]
    pub async fn is_onboarding_complete(&self) -> anyhow::Result<bool> {
        let row = sqlx::query(
            r#"
            SELECT
              (SELECT COUNT(*) FROM hostels) AS hostel_count,
              (SELECT COUNT(*) FROM users) AS staff_count
            "#,
        )
        .fetch_one(&self.inner)
        .await
        .inspect_err(|e| error!(error = %e, "database error checking onboarding status"))?;
        let hostel_count: i64 = row.try_get("hostel_count")?;
        let staff_count: i64 = row.try_get("staff_count")?;
        Ok(hostel_count == 1 && staff_count > 0)
    }

    /// Return all hostels ordered by id.
    #[instrument(skip(self))]
    pub async fn list_hostels(&self) -> anyhow::Result<Vec<Hostel>> {
        debug!("listing hostels");
        let rows = sqlx::query_as::<_, Hostel>(
            "SELECT id, name, address, phone, currency, branding_logo_path, created_at FROM hostels ORDER BY id",
        )
        .fetch_all(&self.inner)
        .await
        .inspect_err(|e| error!(error = %e, "database error listing hostels"))?;
        Ok(rows)
    }

    /// Update a hostel profile and return the updated row.
    ///
    /// Validates that the name is non-empty and the currency is a three-letter
    /// uppercase code.
    #[instrument(skip(self))]
    pub async fn update_hostel(
        &self,
        hostel_id: i64,
        name: String,
        address: Option<String>,
        phone: Option<String>,
        currency: String,
        branding_logo_path: Option<String>,
    ) -> anyhow::Result<Hostel> {
        debug!(hostel_id, "updating hostel");
        if name.trim().is_empty() {
            anyhow::bail!("name is required");
        }
        if currency.len() != 3 || !currency.chars().all(|c| c.is_ascii_uppercase()) {
            anyhow::bail!("currency must be a 3-letter uppercase code");
        }
        let row = sqlx::query_as::<_, Hostel>(
            r#"
            UPDATE hostels
            SET name = ?1, address = ?2, phone = ?3, currency = ?4, branding_logo_path = ?5
            WHERE id = ?6
            RETURNING id, name, address, phone, currency, branding_logo_path, created_at
            "#,
        )
        .bind(&name)
        .bind(&address)
        .bind(&phone)
        .bind(&currency)
        .bind(&branding_logo_path)
        .bind(hostel_id)
        .fetch_one(&self.inner)
        .await
        .inspect_err(|e| error!(error = %e, hostel_id, "database error updating hostel"))?;
        Ok(row)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hash_password;
    use crate::test_support::*;

    #[tokio::test]
    async fn get_hostel_profile_onboarding_flags() {
        let pool = test_pool().await;
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

        // Fresh hostel: no rooms, no admins.
        let profile = pool
            .get_hostel_profile(hostel.id)
            .await
            .expect("fetch profile")
            .expect("hostel exists");
        assert_eq!(profile.room_count, 0);
        assert_eq!(profile.admin_count, 0);
        assert!(!profile.onboarding_complete);

        // Add a room and an admin staff user.
        pool.add_room(NewRoom {
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
        pool.create_user(NewUser {
            hostel_id: hostel.id,
            full_name: "Admin".to_string(),
            email: "admin@example.com".to_string(),
            password_hash: hash_password("pw").expect("hash"),
            role: "admin".to_string(),
        })
        .await
        .expect("add admin");

        let profile = pool
            .get_hostel_profile(hostel.id)
            .await
            .expect("fetch profile")
            .expect("hostel exists");
        assert_eq!(profile.room_count, 1);
        assert_eq!(profile.admin_count, 1);
        assert!(profile.onboarding_complete);
    }

    #[tokio::test]
    async fn complete_onboarding_creates_one_hostel_and_admin_only_once() {
        let pool = test_pool().await;
        let input = CompleteOnboarding {
            hostel: NewHostel {
                name: "Onboarding Hostel".to_string(),
                address: None,
                phone: None,
                currency: Some("UGX".to_string()),
                branding_logo_path: None,
            },
            admin_name: "First Admin".to_string(),
            email: "first.admin@example.com".to_string(),
            password_hash: hash_password("password123").expect("hash"),
        };

        let user = pool
            .complete_onboarding(input.clone())
            .await
            .expect("complete onboarding");

        assert_eq!(user.role, "admin");
        assert_eq!(
            pool.list_hostels().await.expect("list hostels").len(),
            1,
            "onboarding must create exactly one hostel"
        );
        assert_eq!(
            pool.list_users(user.hostel_id)
                .await
                .expect("list staff")
                .len(),
            1,
            "onboarding must create exactly one staff user"
        );

        let err = pool
            .complete_onboarding(input)
            .await
            .expect_err("onboarding must be one-time");
        assert!(err.to_string().contains("already complete"));
    }
}
