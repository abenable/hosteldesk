use crate::model::*;
use crate::{verify_password, DbPool, DUMMY_PASSWORD_HASH};
use sqlx::Row;
use tracing::{debug, error, info, instrument};

impl DbPool {
    /// Insert a new staff user after validating the role.
    #[instrument(skip(self, user))]
    pub async fn create_user(&self, user: NewUser) -> anyhow::Result<i64> {
        if !matches!(user.role.as_str(), "admin" | "manager") {
            anyhow::bail!("role must be 'admin' or 'manager'");
        }

        debug!(hostel_id = user.hostel_id, email = %user.email, "inserting user");
        let id = sqlx::query(
            r#"
            INSERT INTO users (hostel_id, full_name, email, password_hash, role)
            VALUES (?1, ?2, ?3, ?4, ?5)
            "#,
        )
        .bind(user.hostel_id)
        .bind(user.full_name)
        .bind(user.email)
        .bind(user.password_hash)
        .bind(user.role)
        .execute(&self.inner)
        .await
        .inspect_err(|e| error!(error = %e, "database error inserting user"))?
        .last_insert_rowid();

        info!(user_id = id, "user inserted");
        Ok(id)
    }

    /// Fetch a staff user by id without exposing the password hash.
    ///
    /// Returns `None` when the id does not exist.
    #[instrument(skip(self))]
    pub async fn get_user_by_id(&self, user_id: i64) -> anyhow::Result<Option<User>> {
        debug!(user_id, "fetching user by id");
        let user = sqlx::query_as::<_, User>(
            "SELECT id, hostel_id, full_name, email, role, created_at FROM users WHERE id = ?1",
        )
        .bind(user_id)
        .fetch_optional(&self.inner)
        .await
        .inspect_err(|e| error!(error = %e, user_id, "database error fetching user"))?;
        Ok(user)
    }

    /// Authenticate a staff user by email and password.
    ///
    /// Returns the matching user (without their password hash) when credentials are valid.
    /// A single query fetches every candidate row so the number of database
    /// round-trips stays constant regardless of how many hostels share an email.
    #[instrument(skip(self, password))]
    pub async fn authenticate_user(
        &self,
        email: &str,
        password: &str,
    ) -> anyhow::Result<Option<User>> {
        debug!(%email, "authenticating user");

        let rows = sqlx::query(
            r#"
            SELECT id, hostel_id, full_name, email, role, created_at, password_hash
            FROM users
            WHERE email = ?1
            "#,
        )
        .bind(email)
        .fetch_all(&self.inner)
        .await
        .inspect_err(|e| error!(error = %e, %email, "database error fetching user"))?;

        if rows.is_empty() {
            let _ = verify_password(password, DUMMY_PASSWORD_HASH)?;
            debug!(%email, "authentication failed");
            return Ok(None);
        }

        for row in rows {
            let user = User {
                id: row.try_get("id")?,
                hostel_id: row.try_get("hostel_id")?,
                full_name: row.try_get("full_name")?,
                email: row.try_get("email")?,
                role: row.try_get("role")?,
                created_at: row.try_get("created_at")?,
            };
            let hash: String = row.try_get("password_hash")?;

            if verify_password(password, &hash)? {
                info!(user_id = user.id, "user authenticated");
                return Ok(Some(user));
            }
        }

        debug!(%email, "authentication failed");
        Ok(None)
    }

    /// Update a staff user's full name, email, role and optionally their password.
    ///
    /// Returns the updated user row. The caller is responsible for hashing any
    /// supplied password before passing it in.
    #[instrument(skip(self, password_hash))]
    pub async fn update_user(
        &self,
        user_id: i64,
        full_name: String,
        email: String,
        role: String,
        password_hash: Option<String>,
    ) -> anyhow::Result<User> {
        if !matches!(role.as_str(), "admin" | "manager") {
            anyhow::bail!("role must be 'admin' or 'manager'");
        }

        debug!(user_id, %email, "updating user");

        let row = if let Some(hash) = password_hash {
            sqlx::query_as::<_, User>(
                r#"
                UPDATE users
                SET full_name = ?1, email = ?2, role = ?3, password_hash = ?4
                WHERE id = ?5
                RETURNING id, hostel_id, full_name, email, role, created_at
                "#,
            )
            .bind(&full_name)
            .bind(&email)
            .bind(&role)
            .bind(&hash)
            .bind(user_id)
            .fetch_one(&self.inner)
            .await
        } else {
            sqlx::query_as::<_, User>(
                r#"
                UPDATE users
                SET full_name = ?1, email = ?2, role = ?3
                WHERE id = ?4
                RETURNING id, hostel_id, full_name, email, role, created_at
                "#,
            )
            .bind(&full_name)
            .bind(&email)
            .bind(&role)
            .bind(user_id)
            .fetch_one(&self.inner)
            .await
        }
        .inspect_err(|e| error!(error = %e, user_id, "database error updating user"))?;

        info!(user_id = row.id, "user updated");
        Ok(row)
    }

    /// Delete a staff user, protecting against deletion of the last admin.
    ///
    /// Returns `Ok(())` if the user existed and was deleted, or an error if the
    /// user was the only admin remaining.
    #[instrument(skip(self))]
    pub async fn delete_user(&self, user_id: i64) -> anyhow::Result<()> {
        debug!(user_id, "deleting user");

        let user: Option<User> = sqlx::query_as::<_, User>(
            "SELECT id, hostel_id, full_name, email, role, created_at FROM users WHERE id = ?1",
        )
        .bind(user_id)
        .fetch_optional(&self.inner)
        .await
        .inspect_err(|e| error!(error = %e, user_id, "database error fetching user for delete"))?;

        let user = match user {
            Some(u) => u,
            None => return Ok(()),
        };

        if user.role == "admin" {
            let admin_count: i64 = sqlx::query_scalar(
                "SELECT COUNT(*) FROM users WHERE hostel_id = ?1 AND role = 'admin'",
            )
            .bind(user.hostel_id)
            .fetch_one(&self.inner)
            .await
            .inspect_err(|e| error!(error = %e, hostel_id = user.hostel_id, "database error counting admins"))?;

            if admin_count <= 1 {
                anyhow::bail!("cannot delete the only admin account");
            }
        }

        sqlx::query("DELETE FROM users WHERE id = ?1")
            .bind(user_id)
            .execute(&self.inner)
            .await
            .inspect_err(|e| error!(error = %e, user_id, "database error deleting user"))?;

        info!(user_id, "user deleted");
        Ok(())
    }

    /// List staff users for a hostel, excluding password hashes.
    #[instrument(skip(self))]
    pub async fn list_users(&self, hostel_id: i64) -> anyhow::Result<Vec<User>> {
        debug!(hostel_id, "listing users");
        let rows = sqlx::query_as::<_, User>(
            "SELECT id, hostel_id, full_name, email, role, created_at FROM users WHERE hostel_id = ?1 ORDER BY id",
        )
        .bind(hostel_id)
        .fetch_all(&self.inner)
        .await
        .inspect_err(|e| error!(error = %e, hostel_id, "database error listing staff users"))?;
        Ok(rows)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hash_password;
    use crate::test_support::*;

    #[tokio::test]
    async fn authenticate_wrong_password_returns_none() {
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

        pool.create_user(NewUser {
            hostel_id: hostel.id,
            full_name: "Admin".to_string(),
            email: "admin@example.com".to_string(),
            password_hash: hash_password("correct-horse-battery-staple").expect("hash"),
            role: "admin".to_string(),
        })
        .await
        .expect("add admin");

        let user = pool
            .authenticate_user("admin", "wrong_password")
            .await
            .expect("authenticate should not fail");
        assert!(user.is_none(), "wrong password should not authenticate");
    }

    /// An email can exist once per hostel. The single-query authenticator
    /// must still return the user matching the correct password and hostel.
    #[tokio::test]
    async fn authenticate_unknown_email_still_performs_argon2_work() {
        let pool = test_pool().await;
        let started = std::time::Instant::now();

        let user = pool
            .authenticate_user("definitely-not-a-real-email@example.com", "wrong_password")
            .await
            .expect("authenticate unknown email");

        assert!(user.is_none(), "unknown email must not authenticate");
        assert!(
            started.elapsed() >= std::time::Duration::from_millis(1),
            "unknown email must perform Argon2 verification work"
        );
    }

    #[tokio::test]
    async fn authenticate_returns_correct_user_when_email_spans_hostels() {
        let pool = test_pool().await;
        let hostel_a = pool
            .create_hostel(NewHostel {
                name: "Hostel A".to_string(),
                address: None,
                phone: None,
                currency: None,
                branding_logo_path: None,
            })
            .await
            .expect("create hostel a");
        let hostel_b = pool
            .create_hostel(NewHostel {
                name: "Hostel B".to_string(),
                address: None,
                phone: None,
                currency: None,
                branding_logo_path: None,
            })
            .await
            .expect("create hostel b");

        pool.create_user(NewUser {
            hostel_id: hostel_a.id,
            full_name: "A Manager".to_string(),
            email: "manager@example.com".to_string(),
            password_hash: hash_password("password-a").expect("hash a"),
            role: "manager".to_string(),
        })
        .await
        .expect("create user a");
        pool.create_user(NewUser {
            hostel_id: hostel_b.id,
            full_name: "B Manager".to_string(),
            email: "manager@example.com".to_string(),
            password_hash: hash_password("password-b").expect("hash b"),
            role: "manager".to_string(),
        })
        .await
        .expect("create user b");

        let user_b = pool
            .authenticate_user("manager@example.com", "password-b")
            .await
            .expect("authenticate should not fail")
            .expect("should find user b");
        assert_eq!(user_b.hostel_id, hostel_b.id);
        assert_eq!(user_b.full_name, "B Manager");
        assert_eq!(user_b.role, "manager");

        assert!(
            pool.authenticate_user("manager@example.com", "wrong-password")
                .await
                .expect("authenticate should not fail")
                .is_none(),
            "wrong password should return None"
        );
    }

    #[tokio::test]
    async fn update_user_round_trip() {
        let pool = test_pool().await;
        let hostel = seed_hostel(&pool).await;
        let user_id = pool
            .create_user(NewUser {
                hostel_id: hostel.id,
                full_name: "Admin".to_string(),
                email: "admin@example.com".to_string(),
                password_hash: hash_password("oldpw").expect("hash"),
                role: "admin".to_string(),
            })
            .await
            .expect("create staff");

        let updated = pool
            .update_user(
                user_id,
                "Admin Updated".to_string(),
                "admin2".to_string(),
                "manager".to_string(),
                Some(hash_password("newpw").expect("hash")),
            )
            .await
            .expect("update staff");
        assert_eq!(updated.full_name, "Admin Updated");
        assert_eq!(updated.email, "admin2");
        assert_eq!(updated.role, "manager");

        let auth = pool
            .authenticate_user("admin2", "newpw")
            .await
            .expect("authenticate")
            .expect("user exists");
        assert_eq!(auth.id, user_id);
    }

    #[tokio::test]
    async fn delete_user_removes_user_and_protects_last_admin() {
        let pool = test_pool().await;
        let hostel = seed_hostel(&pool).await;

        let admin_id = pool
            .create_user(NewUser {
                hostel_id: hostel.id,
                full_name: "Admin One".to_string(),
                email: "admin1@example.com".to_string(),
                password_hash: hash_password("password123").expect("hash"),
                role: "admin".to_string(),
            })
            .await
            .expect("create admin");

        pool.delete_user(admin_id)
            .await
            .expect_err("deleting the only admin should fail");

        let manager_id = pool
            .create_user(NewUser {
                hostel_id: hostel.id,
                full_name: "Manager One".to_string(),
                email: "manager1@example.com".to_string(),
                password_hash: hash_password("password123").expect("hash"),
                role: "manager".to_string(),
            })
            .await
            .expect("create manager");

        pool.delete_user(manager_id)
            .await
            .expect("deleting a manager should succeed");
        let staff = pool.list_users(hostel.id).await.expect("list staff");
        assert_eq!(staff.len(), 1);

        let admin_two_id = pool
            .create_user(NewUser {
                hostel_id: hostel.id,
                full_name: "Admin Two".to_string(),
                email: "admin2@example.com".to_string(),
                password_hash: hash_password("password123").expect("hash"),
                role: "admin".to_string(),
            })
            .await
            .expect("create second admin");

        pool.delete_user(admin_id)
            .await
            .expect("deleting admin when another exists should succeed");
        let staff = pool.list_users(hostel.id).await.expect("list staff");
        assert_eq!(staff.len(), 1);
        assert_eq!(staff[0].id, admin_two_id);
    }
}
