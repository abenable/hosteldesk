use super::support::*;
use db::{DbPool, NewUser, User};
use serde::Deserialize;
use tauri::State;
use tracing::{info, instrument};

/// Payload for creating a staff login account.
#[derive(Debug, Deserialize)]
pub struct CreateUserRequest {
    pub hostel_id: i64,
    pub full_name: String,
    pub email: String,
    pub password: String,
    pub role: String,
}

/// Payload for staff login.
#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

/// Return the staff user represented by the current backend session.
#[tauri::command]
#[instrument(skip(state, auth_state), fields(command = "get_session"))]
pub async fn get_session(
    state: State<'_, DbPool>,
    auth_state: State<'_, crate::AuthState>,
) -> Result<Option<User>, String> {
    info!("get_session invoked");
    let Some(session) = auth_state.current_session() else {
        return Ok(None);
    };

    let user = state
        .get_user_by_id(session.user_id)
        .await
        .map_err(|e| map_db_error("get_session", e))?;
    if user.is_none() {
        auth_state.clear_current_session();
    }
    Ok(user)
}

/// Clear the current backend session.
#[tauri::command]
#[instrument(skip(auth_state), fields(command = "logout"))]
pub fn logout(auth_state: State<'_, crate::AuthState>) {
    info!("logout invoked");
    auth_state.clear_current_session();
}

/// Create a new staff user and return the generated user id.
#[tauri::command]
#[instrument(
    skip(state, request, license_state, auth_state),
    fields(command = "create_user")
)]
pub async fn create_user(
    request: CreateUserRequest,
    state: State<'_, DbPool>,
    license_state: State<'_, crate::LicenseState>,
    auth_state: State<'_, crate::AuthState>,
) -> Result<i64, String> {
    info!(hostel_id = request.hostel_id, email = %request.email, "create_user invoked");
    require_license(&license_state)?;
    require_admin(&auth_state, request.hostel_id)?;
    validate_password(&request.password)?;

    let password_hash = db::hash_password(&request.password)
        .map_err(|e| format!("Could not hash password: {e}"))?;

    let user = NewUser {
        hostel_id: request.hostel_id,
        full_name: request.full_name,
        email: request.email,
        password_hash,
        role: request.role,
    };

    state
        .create_user(user)
        .await
        .map_err(|e| map_db_error("create_user", e))
}

/// Authenticate a staff user by email and password.
///
/// On success, mints the authoritative in-memory session and returns the user
/// record for display. The returned role is never used for authorization.
#[tauri::command]
#[instrument(skip(state, auth_state, request), fields(command = "login"))]
pub async fn login(
    request: LoginRequest,
    state: State<'_, DbPool>,
    license_state: State<'_, crate::LicenseState>,
    auth_state: State<'_, crate::AuthState>,
) -> Result<User, String> {
    info!(email = %request.email, "login invoked");
    require_license(&license_state)?;

    let user = state
        .authenticate_user(&request.email, &request.password)
        .await
        .map_err(|e| map_db_error("login", e))?
        .ok_or_else(|| "Invalid email or password".to_string())?;

    auth_state.set_current_session(user.id, user.hostel_id, user.role.clone());
    Ok(user)
}

/// Enforce the password policy server-side. The 8-character minimum shown in
/// `TeamSection.tsx` is a UX hint only — without this, `create_user` /
/// `update_user` accept any password, since the backend never re-checks it.
fn validate_password(password: &str) -> Result<(), String> {
    if password.chars().count() < 8 {
        return Err("Password must be at least 8 characters.".to_string());
    }
    Ok(())
}

/// Request payload for [`delete_user`].
#[derive(Debug, Deserialize)]
pub struct DeleteUserRequest {
    pub user_id: i64,
}

/// Request payload for [`list_users`].
#[derive(Debug, Deserialize)]
pub struct ListUsersRequest {
    pub hostel_id: i64,
}

/// List staff users for a hostel.
#[tauri::command]
#[instrument(skip(state, license_state), fields(command = "list_users"))]
pub async fn list_users(
    request: ListUsersRequest,
    state: State<'_, DbPool>,
    license_state: State<'_, crate::LicenseState>,
    auth_state: State<'_, crate::AuthState>,
) -> Result<Vec<User>, String> {
    info!(hostel_id = request.hostel_id, "list_users invoked");
    require_license(&license_state)?;
    require_admin(&auth_state, request.hostel_id)?;
    state
        .list_users(request.hostel_id)
        .await
        .map_err(|e| map_db_error("list_users", e))
}

/// Request payload for [`update_user`].
#[derive(Debug, Deserialize)]
pub struct UpdateUserRequest {
    pub user_id: i64,
    pub full_name: String,
    pub email: String,
    pub role: String,
    pub password: Option<String>,
}

/// Update a staff user's profile and optionally their password.
#[tauri::command]
#[instrument(
    skip(state, request, license_state, auth_state),
    fields(command = "update_user")
)]
pub async fn update_user(
    request: UpdateUserRequest,
    state: State<'_, DbPool>,
    license_state: State<'_, crate::LicenseState>,
    auth_state: State<'_, crate::AuthState>,
) -> Result<User, String> {
    info!(user_id = request.user_id, "update_user invoked");
    require_license(&license_state)?;
    let target = state
        .get_user_by_id(request.user_id)
        .await
        .map_err(|e| map_db_error("update_user", e))?
        .ok_or_else(|| "user not found".to_string())?;
    let session = require_session(&auth_state, target.hostel_id)?;
    let role_changed = target.role != request.role;
    // Changing any role, or editing someone else's account, requires admin.
    // A non-admin may still update their own name/email/password as long as
    // their role stays the same — this is how a manager rotates their own
    // password (AccountSection.tsx), which a blanket self-update admin gate
    // used to block.
    if session.role != "admin" && (role_changed || request.user_id != session.user_id) {
        return Err("forbidden".to_string());
    }

    let password_hash = match request.password {
        Some(ref password) => {
            validate_password(password)?;
            Some(db::hash_password(password).map_err(|e| format!("Could not hash password: {e}"))?)
        }
        None => None,
    };

    state
        .update_user(
            request.user_id,
            request.full_name,
            request.email,
            request.role,
            password_hash,
        )
        .await
        .map_err(|e| map_db_error("update_user", e))
}

/// Delete a staff user, refusing to remove the last admin of a hostel.
#[tauri::command]
#[instrument(
    skip(state, request, license_state, auth_state),
    fields(command = "delete_user")
)]
pub async fn delete_user(
    request: DeleteUserRequest,
    state: State<'_, DbPool>,
    license_state: State<'_, crate::LicenseState>,
    auth_state: State<'_, crate::AuthState>,
) -> Result<(), String> {
    info!(user_id = request.user_id, "delete_user invoked");
    require_license(&license_state)?;
    let target = state
        .get_user_by_id(request.user_id)
        .await
        .map_err(|e| map_db_error("delete_user", e))?
        .ok_or_else(|| "user not found".to_string())?;
    require_admin(&auth_state, target.hostel_id)?;
    state
        .delete_user(request.user_id)
        .await
        .map_err(|e| map_db_error("delete_user", e))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::test_support::{setup_license_state, temp_db_url};
    use crate::{commands, AuthState, LicenseState};
    use tauri::Manager;

    async fn setup_staff_user(
        pool: &DbPool,
        full_name: &str,
        email: &str,
        role: &str,
    ) -> (i64, String) {
        let password = format!("password_{email}");
        let id = pool
            .create_user(db::NewUser {
                hostel_id: 1,
                full_name: full_name.to_string(),
                email: email.to_string(),
                password_hash: db::hash_password(&password).expect("hash"),
                role: role.to_string(),
            })
            .await
            .expect("create staff user");
        (id, password)
    }

    #[test]
    fn password_hash_and_verify_round_trip() {
        let password = "a_secure_password_123";
        let hash = db::hash_password(password).expect("hashing should succeed");
        assert!(db::verify_password(password, &hash).expect("verification should not fail"));
        assert!(
            !db::verify_password("wrong_password", &hash).expect("verification should not fail")
        );
    }

    #[tokio::test]
    async fn create_and_login_user_round_trip() {
        let url = temp_db_url();
        let pool = DbPool::open(&url).await.expect("open database");

        // A staff user requires an existing hostel because of the foreign key.
        sqlx::query("INSERT INTO hostels (name, currency) VALUES (?1, ?2)")
            .bind("Test Hostel")
            .bind("USD")
            .execute(&pool.inner)
            .await
            .expect("insert hostel");
        let hostel_id = sqlx::query_scalar::<_, i64>("SELECT id FROM hostels WHERE name = ?1")
            .bind("Test Hostel")
            .fetch_one(&pool.inner)
            .await
            .expect("fetch hostel id");

        let password = "frontdesk_2024";
        let id = pool
            .create_user(db::NewUser {
                hostel_id,
                full_name: "Test User".to_string(),
                email: "testuser@example.com".to_string(),
                password_hash: db::hash_password(password).expect("hash"),
                role: "manager".to_string(),
            })
            .await
            .expect("create staff user");
        assert!(id > 0);

        let user = pool
            .authenticate_user("testuser@example.com", password)
            .await
            .expect("authenticate")
            .expect("user should exist");
        assert_eq!(user.email, "testuser@example.com");
        assert_eq!(user.role, "manager");

        let none = pool
            .authenticate_user("testuser@example.com", "wrong_password")
            .await
            .expect("authenticate");
        assert!(none.is_none());
    }

    #[tokio::test]
    async fn manager_session_cannot_delete_user() {
        let url = temp_db_url();
        let pool = DbPool::open(&url).await.expect("open database");

        sqlx::query("INSERT INTO hostels (name, currency) VALUES (?1, ?2)")
            .bind("Auth Test Hostel")
            .bind("USD")
            .execute(&pool.inner)
            .await
            .expect("insert hostel");

        let (admin_id, _admin_password) = setup_staff_user(&pool, "Admin", "admin", "admin").await;
        let (manager_id, _manager_password) =
            setup_staff_user(&pool, "Manager", "manager", "manager").await;

        let license_state = setup_license_state();
        let auth_state = AuthState::new();
        auth_state.set_current_session(manager_id, 1, "manager");

        let app = tauri::test::mock_app();
        app.manage(pool);
        app.manage(license_state);
        app.manage(auth_state);

        let result = commands::delete_user(
            commands::DeleteUserRequest { user_id: admin_id },
            app.state::<DbPool>(),
            app.state::<LicenseState>(),
            app.state::<AuthState>(),
        )
        .await;

        let err = result.expect_err("manager should not be able to delete staff users");
        assert!(
            err.contains("forbidden") || err.contains("unauthorized"),
            "expected authorization error, got: {err}"
        );

        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users WHERE id = ?1")
            .bind(admin_id)
            .fetch_one(&app.state::<DbPool>().inner)
            .await
            .expect("count admin user");
        assert_eq!(count, 1, "admin should not have been deleted");
    }

    /// Staff listing exposes every user's email and role in the hostel; it
    /// must be admin-gated like its create/update/delete siblings, not open
    /// to any authenticated session.
    #[tokio::test]
    async fn manager_session_cannot_list_users() {
        let url = temp_db_url();
        let pool = DbPool::open(&url).await.expect("open database");

        sqlx::query("INSERT INTO hostels (name, currency) VALUES (?1, ?2)")
            .bind("Auth Test Hostel")
            .bind("USD")
            .execute(&pool.inner)
            .await
            .expect("insert hostel");

        let (_admin_id, _admin_password) = setup_staff_user(&pool, "Admin", "admin", "admin").await;
        let (manager_id, _manager_password) =
            setup_staff_user(&pool, "Manager", "manager", "manager").await;

        let license_state = setup_license_state();
        let auth_state = AuthState::new();
        auth_state.set_current_session(manager_id, 1, "manager");

        let app = tauri::test::mock_app();
        app.manage(pool);
        app.manage(license_state);
        app.manage(auth_state);

        let result = commands::list_users(
            commands::ListUsersRequest { hostel_id: 1 },
            app.state::<DbPool>(),
            app.state::<LicenseState>(),
            app.state::<AuthState>(),
        )
        .await;

        let err = result.expect_err("manager should not be able to list staff users");
        assert!(
            err.contains("forbidden") || err.contains("unauthorized"),
            "expected authorization error, got: {err}"
        );
    }

    #[tokio::test]
    async fn manager_session_cannot_escalate_own_role() {
        let url = temp_db_url();
        let pool = DbPool::open(&url).await.expect("open database");

        sqlx::query("INSERT INTO hostels (name, currency) VALUES (?1, ?2)")
            .bind("Auth Test Hostel")
            .bind("USD")
            .execute(&pool.inner)
            .await
            .expect("insert hostel");

        let (admin_id, _admin_password) = setup_staff_user(&pool, "Admin", "admin", "admin").await;
        let (manager_id, _manager_password) =
            setup_staff_user(&pool, "Manager", "manager", "manager").await;

        let license_state = setup_license_state();
        let auth_state = AuthState::new();
        auth_state.set_current_session(manager_id, 1, "manager");

        let app = tauri::test::mock_app();
        app.manage(pool);
        app.manage(license_state);
        app.manage(auth_state);

        let result = commands::update_user(
            commands::UpdateUserRequest {
                user_id: manager_id,
                full_name: "Manager".to_string(),
                email: "manager".to_string(),
                role: "admin".to_string(),
                password: None,
            },
            app.state::<DbPool>(),
            app.state::<LicenseState>(),
            app.state::<AuthState>(),
        )
        .await;

        let err = result.expect_err("manager should not be able to escalate their role");
        assert!(
            err.contains("forbidden") || err.contains("unauthorized"),
            "expected authorization error, got: {err}"
        );

        let role: String = sqlx::query_scalar("SELECT role FROM users WHERE id = ?1")
            .bind(manager_id)
            .fetch_one(&app.state::<DbPool>().inner)
            .await
            .expect("fetch manager role");
        assert_eq!(role, "manager", "manager role should remain unchanged");

        // Ensure the change did not inadvertently land on the admin.
        let admin_role: String = sqlx::query_scalar("SELECT role FROM users WHERE id = ?1")
            .bind(admin_id)
            .fetch_one(&app.state::<DbPool>().inner)
            .await
            .expect("fetch admin role");
        assert_eq!(admin_role, "admin");
    }

    /// A manager must be able to rotate their own password (AccountSection.tsx)
    /// without being an admin, as long as they aren't also changing their role.
    #[tokio::test]
    async fn manager_session_can_update_own_password() {
        let url = temp_db_url();
        let pool = DbPool::open(&url).await.expect("open database");

        sqlx::query("INSERT INTO hostels (name, currency) VALUES (?1, ?2)")
            .bind("Auth Test Hostel")
            .bind("USD")
            .execute(&pool.inner)
            .await
            .expect("insert hostel");

        let (manager_id, _manager_password) =
            setup_staff_user(&pool, "Manager", "manager@example.com", "manager").await;

        let license_state = setup_license_state();
        let auth_state = AuthState::new();
        auth_state.set_current_session(manager_id, 1, "manager");

        let app = tauri::test::mock_app();
        app.manage(pool);
        app.manage(license_state);
        app.manage(auth_state);

        let result = commands::update_user(
            commands::UpdateUserRequest {
                user_id: manager_id,
                full_name: "Manager".to_string(),
                email: "manager@example.com".to_string(),
                role: "manager".to_string(),
                password: Some("a-new-password".to_string()),
            },
            app.state::<DbPool>(),
            app.state::<LicenseState>(),
            app.state::<AuthState>(),
        )
        .await;

        result.expect("manager should be able to change their own password");
    }
}
