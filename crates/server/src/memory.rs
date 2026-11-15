//! Process-local store for tests and the desk's two-desk sync test.
//!
// ponytail: no session expiry or sign-in lockout here; PgStore enforces both
// in SQL, and scripts/smoke-worker.sh covers them.

use crate::{
    apply_batch, hash_password, normalize_email, random_device_prefix, random_id, route,
    token_hash, verify_password, ApiError, Reply, SyncStore, DUMMY_HASH, TRIAL_DAYS,
};
use chrono::NaiveDate;
use protocol::{
    AuthResponse, EnrollRequest, EnrollResponse, Entity, Ledger, LicenseStatus, PullRequest,
    PullResponse, PushRequest, PushResponse, SignInRequest, SignUpRequest, PULL_PAGE_SIZE,
};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};

struct Account {
    org_id: String,
    password_hash: String,
}

struct HostelData {
    org_id: String,
    ledger: Ledger,
    changes: Vec<(i64, Entity)>,
}

struct License {
    name: String,
    plan: String,
    expires_on: NaiveDate,
}

#[derive(Default)]
struct Inner {
    accounts: HashMap<String, Account>,
    licenses: HashMap<String, License>,
    sessions: HashMap<String, String>,
    devices: HashMap<String, (String, String)>,
    hostels: HashMap<String, HostelData>,
}

/// In-memory [`SyncStore`]. One lock serializes every request.
#[derive(Default)]
pub struct MemoryStore {
    inner: Mutex<Inner>,
}

impl MemoryStore {
    /// Empty store.
    pub fn new() -> Self {
        Self::default()
    }

    /// Serve one request the way the Worker does.
    pub async fn handle(
        &self,
        method: &str,
        path: &str,
        authorization: Option<&str>,
        body: &[u8],
    ) -> Reply {
        route(self, method, path, authorization, body).await
    }

    /// Set an organization's license, as `scripts/license-account.sh` does in Postgres.
    pub fn set_license(
        &self,
        org_id: &str,
        plan: &str,
        expires_on: NaiveDate,
    ) -> Result<(), ApiError> {
        let mut inner = self.lock()?;
        let license = inner
            .licenses
            .get_mut(org_id)
            .ok_or_else(|| ApiError::BadRequest("no such organization".into()))?;
        license.plan = plan.to_string();
        license.expires_on = expires_on;
        Ok(())
    }

    fn lock(&self) -> Result<MutexGuard<'_, Inner>, ApiError> {
        self.inner
            .lock()
            .map_err(|e| ApiError::Storage(e.to_string()))
    }

    fn session(inner: &mut Inner, org_id: String) -> AuthResponse {
        let access_token = random_id();
        inner
            .sessions
            .insert(token_hash(&access_token), org_id.clone());
        AuthResponse {
            access_token,
            organization_id: org_id,
        }
    }

    fn device_org(inner: &Inner, refresh_token: &str) -> Result<String, ApiError> {
        inner
            .devices
            .get(&token_hash(refresh_token))
            .map(|(org_id, _)| org_id.clone())
            .ok_or(ApiError::Unauthorized)
    }
}

impl SyncStore for MemoryStore {
    async fn sign_up(&self, request: SignUpRequest) -> Result<AuthResponse, ApiError> {
        let email = normalize_email(&request.email);
        let password_hash = hash_password(&request.password)?;
        let mut inner = self.lock()?;
        if inner.accounts.contains_key(&email) {
            return Err(ApiError::BadRequest("email is already registered".into()));
        }
        let org_id = random_id();
        inner.licenses.insert(
            org_id.clone(),
            License {
                name: request.organization_name.trim().to_string(),
                plan: "trial".into(),
                expires_on: chrono::Utc::now().date_naive() + chrono::Duration::days(TRIAL_DAYS),
            },
        );
        inner.accounts.insert(
            email,
            Account {
                org_id: org_id.clone(),
                password_hash,
            },
        );
        Ok(Self::session(&mut inner, org_id))
    }

    async fn sign_in(&self, request: SignInRequest) -> Result<AuthResponse, ApiError> {
        let email = normalize_email(&request.email);
        let mut inner = self.lock()?;
        let Some(account) = inner.accounts.get(&email) else {
            verify_password(&request.password, DUMMY_HASH);
            return Err(ApiError::Unauthorized);
        };
        if !verify_password(&request.password, &account.password_hash) {
            return Err(ApiError::Unauthorized);
        }
        let org_id = account.org_id.clone();
        Ok(Self::session(&mut inner, org_id))
    }

    async fn enroll(
        &self,
        access_token: &str,
        _request: EnrollRequest,
    ) -> Result<EnrollResponse, ApiError> {
        let mut inner = self.lock()?;
        let org_id = inner
            .sessions
            .get(&token_hash(access_token))
            .cloned()
            .ok_or(ApiError::Unauthorized)?;
        let device_prefix = loop {
            let prefix = random_device_prefix();
            if !inner
                .devices
                .values()
                .any(|(org, used)| *org == org_id && *used == prefix)
            {
                break prefix;
            }
        };
        let refresh_token = random_id();
        inner.devices.insert(
            token_hash(&refresh_token),
            (org_id.clone(), device_prefix.clone()),
        );
        let mut hostel_public_ids: Vec<String> = inner
            .hostels
            .iter()
            .filter(|(_, hostel)| hostel.org_id == org_id)
            .map(|(id, _)| id.clone())
            .collect();
        hostel_public_ids.sort();
        Ok(EnrollResponse {
            device_id: random_id(),
            device_prefix,
            refresh_token,
            hostel_public_ids,
        })
    }

    async fn push(
        &self,
        refresh_token: &str,
        request: PushRequest,
    ) -> Result<PushResponse, ApiError> {
        let mut inner = self.lock()?;
        let org_id = Self::device_org(&inner, refresh_token)?;
        let hostel = inner
            .hostels
            .entry(request.hostel_public_id.clone())
            .or_insert_with(|| HostelData {
                org_id: org_id.clone(),
                ledger: Ledger::default(),
                changes: Vec::new(),
            });
        if hostel.org_id != org_id {
            return Err(ApiError::Unauthorized);
        }
        let next_cursor = hostel.changes.last().map_or(1, |(cursor, _)| cursor + 1);
        let outcome = apply_batch(
            &mut hostel.ledger,
            &request.hostel_public_id,
            &request.commands,
            next_cursor,
        );
        hostel.changes.extend(outcome.changes);
        Ok(PushResponse {
            results: outcome.results,
        })
    }

    async fn pull(
        &self,
        refresh_token: &str,
        request: PullRequest,
    ) -> Result<PullResponse, ApiError> {
        let inner = self.lock()?;
        let org_id = Self::device_org(&inner, refresh_token)?;
        let Some(hostel) = inner.hostels.get(&request.hostel_public_id) else {
            return Ok(PullResponse {
                cursor: request.cursor,
                changes: Vec::new(),
                has_more: false,
            });
        };
        if hostel.org_id != org_id {
            return Err(ApiError::Unauthorized);
        }
        let mut page: Vec<&(i64, Entity)> = hostel
            .changes
            .iter()
            .filter(|(at, _)| *at > request.cursor)
            .take(PULL_PAGE_SIZE as usize + 1)
            .collect();
        let has_more = page.len() > PULL_PAGE_SIZE as usize;
        page.truncate(PULL_PAGE_SIZE as usize);
        let cursor = page.last().map_or(request.cursor, |(at, _)| *at);
        Ok(PullResponse {
            cursor,
            changes: page.into_iter().map(|(_, entity)| entity.clone()).collect(),
            has_more,
        })
    }

    async fn license(&self, refresh_token: &str) -> Result<LicenseStatus, ApiError> {
        let inner = self.lock()?;
        let org_id = Self::device_org(&inner, refresh_token)?;
        let license = inner
            .licenses
            .get(&org_id)
            .ok_or_else(|| ApiError::Storage("organization has no license row".into()))?;
        Ok(LicenseStatus {
            account_name: license.name.clone(),
            plan: license.plan.clone(),
            expires_on: license.expires_on,
        })
    }
}

/// Serve `store` over plain HTTP/1.1 on an already-bound listener, one
/// request per connection. Test harness for the desk's sync tests only.
//
// ponytail: hand-rolled because it only has to understand reqwest's JSON
// POSTs; the deployed API is the Worker in d1.rs.
pub async fn serve_memory(
    listener: tokio::net::TcpListener,
    store: Arc<MemoryStore>,
) -> std::io::Result<()> {
    loop {
        let (stream, _) = listener.accept().await?;
        let store = store.clone();
        tokio::spawn(async move {
            let _ = answer(stream, &store).await;
        });
    }
}

async fn answer(stream: tokio::net::TcpStream, store: &MemoryStore) -> std::io::Result<()> {
    let mut reader = BufReader::new(stream);
    let mut request_line = String::new();
    reader.read_line(&mut request_line).await?;
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or_default().to_string();
    let path = parts.next().unwrap_or_default().to_string();
    let (mut length, mut authorization) = (0usize, None);
    loop {
        let mut line = String::new();
        reader.read_line(&mut line).await?;
        let line = line.trim_end();
        if line.is_empty() {
            break;
        }
        if let Some((name, value)) = line.split_once(':') {
            match name.trim().to_ascii_lowercase().as_str() {
                "content-length" => length = value.trim().parse().unwrap_or(0),
                "authorization" => authorization = Some(value.trim().to_string()),
                _ => {}
            }
        }
    }
    let mut body = vec![0; length.min(crate::MAX_BODY_BYTES)];
    reader.read_exact(&mut body).await?;
    let reply = store
        .handle(&method, &path, authorization.as_deref(), &body)
        .await;
    let response = format!(
        "HTTP/1.1 {} X\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
        reply.status,
        reply.body.len(),
        reply.body
    );
    reader.get_mut().write_all(response.as_bytes()).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use protocol::{AckStatus, CommandEnvelope, LicenseStatus, SyncCommand};

    async fn call(
        app: &MemoryStore,
        uri: &str,
        token: Option<&str>,
        body: serde_json::Value,
    ) -> (u16, serde_json::Value) {
        let auth = token.map(|token| format!("Bearer {token}"));
        let reply = app
            .handle("POST", uri, auth.as_deref(), body.to_string().as_bytes())
            .await;
        let json = serde_json::from_str(&reply.body).unwrap_or(serde_json::Value::Null);
        (reply.status, json)
    }

    fn room(id: &str, number: &str) -> SyncCommand {
        SyncCommand::UpsertRoom {
            public_id: id.into(),
            hostel_public_id: "h1".into(),
            base_revision: 0,
            room_number: number.into(),
            room_type: "single".into(),
            capacity: 1,
            floor: None,
            semester_rate: "10".into(),
            recess_rate: "5".into(),
            status: "vacant".into(),
        }
    }

    fn check_in(allocation: &str, resident: &str) -> SyncCommand {
        SyncCommand::CheckIn {
            allocation_public_id: allocation.into(),
            hostel_public_id: "h1".into(),
            resident_public_id: resident.into(),
            room_public_id: "r1".into(),
            full_name: "Ada".into(),
            phone: None,
            email: None,
            id_number: None,
            next_of_kin: None,
            next_of_kin_phone: None,
            rate_agreed: "10".into(),
            term: "semester_1".into(),
            academic_year: "2026/2027".into(),
            checked_in_at: "2026-11-15 09:00:00".into(),
        }
    }

    fn envelope(key: &str, command: SyncCommand) -> CommandEnvelope {
        CommandEnvelope {
            idempotency_key: key.into(),
            command,
        }
    }

    /// Sign up and enroll; returns (org id, device refresh token).
    async fn enrolled(app: &MemoryStore) -> (String, String) {
        let (status, auth) = call(
            app,
            "/v1/sign-up",
            None,
            serde_json::json!({"organization_name":"North","email":" A@B.co ","password":"secret-pass"}),
        )
        .await;
        assert_eq!(status, 200);
        let token = auth["access_token"].as_str().unwrap().to_string();
        let (status, device) = call(
            app,
            "/v1/devices",
            Some(&token),
            serde_json::json!({"device_name":"front desk"}),
        )
        .await;
        assert_eq!(status, 200);
        assert_eq!(device["device_prefix"].as_str().unwrap().len(), 6);
        (
            auth["organization_id"].as_str().unwrap().to_string(),
            device["refresh_token"].as_str().unwrap().to_string(),
        )
    }

    #[tokio::test]
    async fn push_acks_warns_and_rejects_then_pull_pages() {
        let app = MemoryStore::new();
        let (_, refresh) = enrolled(&app).await;
        let mut foreign = room("r9", "9");
        if let SyncCommand::UpsertRoom {
            hostel_public_id, ..
        } = &mut foreign
        {
            *hostel_public_id = "someone-else".into();
        }
        let body = PushRequest {
            hostel_public_id: "h1".into(),
            commands: vec![
                envelope("1", room("r1", "1")),
                envelope("2", check_in("a1", "p1")),
                envelope("3", check_in("a2", "p2")),
                envelope("4", foreign),
            ],
        };
        let (status, pushed) = call(
            &app,
            "/v1/push",
            Some(&refresh),
            serde_json::to_value(&body).unwrap(),
        )
        .await;
        assert_eq!(status, 200);
        let pushed: PushResponse = serde_json::from_value(pushed).unwrap();
        let statuses: Vec<AckStatus> = pushed.results.iter().map(|r| r.status).collect();
        assert_eq!(
            statuses,
            [
                AckStatus::Acked,
                AckStatus::Acked,
                AckStatus::Acked,
                AckStatus::Conflict
            ]
        );
        assert!(pushed.results[2]
            .message
            .as_deref()
            .unwrap()
            .contains("over capacity"));

        let (status, pulled) = call(
            &app,
            "/v1/pull",
            Some(&refresh),
            serde_json::json!({"hostel_public_id":"h1","cursor":0}),
        )
        .await;
        assert_eq!(status, 200);
        let pulled: PullResponse = serde_json::from_value(pulled).unwrap();
        assert!(!pulled.has_more);
        assert_eq!(pulled.cursor, pulled.changes.len() as i64);
    }

    #[tokio::test]
    async fn oversized_push_is_rejected_and_bad_token_is_401() {
        let app = MemoryStore::new();
        let (_, refresh) = enrolled(&app).await;
        let commands: Vec<_> = (0..=crate::MAX_PUSH_COMMANDS)
            .map(|n| envelope(&n.to_string(), room(&format!("r{n}"), &n.to_string())))
            .collect();
        let body = serde_json::json!({"hostel_public_id":"h1","commands":commands});
        let (status, _) = call(&app, "/v1/push", Some(&refresh), body).await;
        assert_eq!(status, 400);
        let (status, _) = call(
            &app,
            "/v1/pull",
            Some("not-a-token"),
            serde_json::json!({"hostel_public_id":"h1","cursor":0}),
        )
        .await;
        assert_eq!(status, 401);
    }

    /// The account is the license, and only the server decides it: a new
    /// account is on a trial straight away, before it has any hostel.
    #[tokio::test]
    async fn a_new_account_is_on_a_trial_and_the_server_reports_it() {
        let app = MemoryStore::new();
        let (org_id, refresh) = enrolled(&app).await;
        let (status, license) =
            call(&app, "/v1/license", Some(&refresh), serde_json::json!({})).await;
        assert_eq!(status, 200);
        let license: LicenseStatus = serde_json::from_value(license).unwrap();
        let today = chrono::Utc::now().date_naive();
        assert_eq!(
            license,
            LicenseStatus {
                account_name: "North".into(),
                plan: "trial".into(),
                expires_on: today + chrono::Duration::days(crate::TRIAL_DAYS),
            }
        );

        app.set_license(&org_id, "1-year", today - chrono::Duration::days(1))
            .unwrap();
        let (_, license) = call(&app, "/v1/license", Some(&refresh), serde_json::json!({})).await;
        let license: LicenseStatus = serde_json::from_value(license).unwrap();
        assert!(
            license.expires_on < today,
            "the desk locks on an ended plan"
        );

        let (status, _) = call(&app, "/v1/license", Some("forged"), serde_json::json!({})).await;
        assert_eq!(status, 401);
    }

    #[tokio::test]
    async fn sign_in_is_case_insensitive_and_rejects_wrong_password() {
        let app = MemoryStore::new();
        enrolled(&app).await;
        let (status, _) = call(
            &app,
            "/v1/sign-in",
            None,
            serde_json::json!({"email":"a@b.co","password":"secret-pass"}),
        )
        .await;
        assert_eq!(status, 200);
        let (status, _) = call(
            &app,
            "/v1/sign-in",
            None,
            serde_json::json!({"email":"a@b.co","password":"wrong-pass"}),
        )
        .await;
        assert_eq!(status, 401);
        let (status, _) = call(
            &app,
            "/v1/sign-up",
            None,
            serde_json::json!({"organization_name":"X","email":"a@b.co","password":"short"}),
        )
        .await;
        assert_eq!(status, 400);
    }
}
