//! Hosted multi-tenant API.
//!
//! Desks enroll a device, push idempotent commands, and pull rows changed
//! since a cursor. The in-memory store is what tests run. [`PostgresStore`]
//! is what a deployed process uses.

mod pg;

use argon2::password_hash::{
    rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString,
};
use argon2::Argon2;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::routing::{get, post};
use axum::{Json, Router};
use base64::Engine;
use ed25519_dalek::{Signer, SigningKey};
use protocol::{ApplyOutcome, Entity, Ledger, SyncCommand};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

pub use pg::PostgresStore;

/// Account creation.
#[derive(Debug, Deserialize)]
pub struct SignUpRequest {
    /// Customer name.
    pub organization_name: String,
    /// Login email.
    pub email: String,
    /// Plain password. Stored only as an Argon2 hash.
    pub password: String,
}

/// Account sign-in.
#[derive(Debug, Deserialize)]
pub struct SignInRequest {
    /// Login email.
    pub email: String,
    /// Plain password.
    pub password: String,
}

/// Session returned to a signed-in account. The token authorizes device enrollment.
#[derive(Debug, Serialize, Deserialize)]
pub struct AuthResponse {
    /// Bearer token for `/v1/devices`.
    pub access_token: String,
    /// Organization id.
    pub organization_id: String,
}

/// Name the desk wants to show for itself.
#[derive(Debug, Deserialize)]
pub struct EnrollRequest {
    /// Human-readable device name.
    pub device_name: String,
}

/// Credentials the desk stores in the OS keyring.
#[derive(Debug, Serialize, Deserialize)]
pub struct EnrollResponse {
    /// Stable device id.
    pub device_id: String,
    /// Short prefix used in receipt numbers, for example `AB12`.
    pub device_prefix: String,
    /// Bearer token for push and pull. This is the device refresh token.
    pub refresh_token: String,
    /// Hostels this organization already has on the server.
    ///
    /// A second desk uses the first id to pull before it has a local copy.
    /// Empty when this device is the first one to enroll.
    #[serde(default)]
    pub hostel_public_ids: Vec<String>,
}

/// A batch of commands from one desk.
#[derive(Debug, Deserialize)]
pub struct PushRequest {
    /// Hostel the commands belong to.
    pub hostel_public_id: String,
    /// Commands in the order they happened locally.
    pub commands: Vec<protocol::CommandEnvelope>,
}

/// Per-command acknowledgement.
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct CommandAck {
    /// The key the desk sent.
    pub idempotency_key: String,
    /// `acked` or `conflict`.
    pub status: String,
    /// Conflict message, when `status` is `conflict`.
    pub message: Option<String>,
}

/// Result of a push.
#[derive(Debug, Serialize, Deserialize)]
pub struct PushResponse {
    /// One entry per command, in request order.
    pub results: Vec<CommandAck>,
}

/// Result of a pull.
#[derive(Debug, Serialize, Deserialize)]
pub struct PullResponse {
    /// Cursor to send on the next pull.
    pub cursor: i64,
    /// Rows changed after the requested cursor.
    pub changes: Vec<Entity>,
    /// Signed offline lease, when the server has a signing key.
    pub lease: Option<String>,
}

/// Failures returned as HTTP errors.
#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    /// Email and password did not match, or the token is missing.
    #[error("unauthorized")]
    Unauthorized,
    /// The body failed a check that is not a sync conflict.
    #[error("{0}")]
    BadRequest(String),
    /// Storage failed.
    #[error("{0}")]
    Storage(String),
}

impl axum::response::IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        let status = match self {
            ApiError::Unauthorized => StatusCode::UNAUTHORIZED,
            ApiError::BadRequest(_) => StatusCode::BAD_REQUEST,
            ApiError::Storage(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };
        (status, self.to_string()).into_response()
    }
}

/// What the HTTP layer needs from either store.
#[async_trait::async_trait]
pub trait SyncStore: Send + Sync {
    /// Create an organization and its first account.
    async fn sign_up(&self, request: SignUpRequest) -> Result<AuthResponse, ApiError>;
    /// Sign an existing account in.
    async fn sign_in(&self, request: SignInRequest) -> Result<AuthResponse, ApiError>;
    /// Enroll a desktop. `access_token` is the account token from sign-in.
    async fn enroll(
        &self,
        access_token: &str,
        request: EnrollRequest,
    ) -> Result<EnrollResponse, ApiError>;
    /// Apply a batch of commands for the device that owns `refresh_token`.
    async fn push(
        &self,
        refresh_token: &str,
        request: PushRequest,
    ) -> Result<PushResponse, ApiError>;
    /// Rows changed after `cursor` for one hostel.
    async fn pull(
        &self,
        refresh_token: &str,
        hostel_public_id: &str,
        cursor: i64,
    ) -> Result<PullResponse, ApiError>;
}

#[derive(Clone)]
struct Org {
    id: String,
    accounts: HashMap<String, String>,
}

struct Device {
    org_id: String,
}

struct HostelData {
    org_id: String,
    ledger: Ledger,
    changes: Vec<(i64, Entity)>,
    next_cursor: i64,
}

/// Process-local store. Tests and a single-node demo use this.
#[derive(Default)]
pub struct MemoryStore {
    orgs: Mutex<HashMap<String, Org>>,
    sessions: Mutex<HashMap<String, String>>,
    devices: Mutex<HashMap<String, Device>>,
    hostels: Mutex<HashMap<String, HostelData>>,
}

impl MemoryStore {
    /// Empty store.
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait::async_trait]
impl SyncStore for MemoryStore {
    async fn sign_up(&self, request: SignUpRequest) -> Result<AuthResponse, ApiError> {
        if request.password.chars().count() < 8 {
            return Err(ApiError::BadRequest(
                "password must be at least 8 characters".into(),
            ));
        }
        let mut orgs = self
            .orgs
            .lock()
            .map_err(|e| ApiError::Storage(e.to_string()))?;
        if orgs
            .values()
            .any(|org| org.accounts.contains_key(&request.email))
        {
            return Err(ApiError::BadRequest("email is already registered".into()));
        }
        let org_id = random_id();
        let hash =
            hash_password(&request.password).map_err(|e| ApiError::Storage(e.to_string()))?;
        orgs.insert(
            org_id.clone(),
            Org {
                id: org_id.clone(),
                accounts: HashMap::from([(request.email, hash)]),
            },
        );
        drop(orgs);
        Ok(self.session_for(org_id)?)
    }

    async fn sign_in(&self, request: SignInRequest) -> Result<AuthResponse, ApiError> {
        let orgs = self
            .orgs
            .lock()
            .map_err(|e| ApiError::Storage(e.to_string()))?;
        let org = orgs
            .values()
            .find(|org| org.accounts.contains_key(&request.email));
        let Some(org) = org else {
            let _ = verify_password(&request.password, DUMMY_HASH);
            return Err(ApiError::Unauthorized);
        };
        let Some(hash) = org.accounts.get(&request.email) else {
            return Err(ApiError::Unauthorized);
        };
        if !verify_password(&request.password, hash).unwrap_or(false) {
            return Err(ApiError::Unauthorized);
        }
        let org_id = org.id.clone();
        drop(orgs);
        Ok(self.session_for(org_id)?)
    }

    async fn enroll(
        &self,
        access_token: &str,
        _request: EnrollRequest,
    ) -> Result<EnrollResponse, ApiError> {
        let org_id = self
            .sessions
            .lock()
            .map_err(|e| ApiError::Storage(e.to_string()))?
            .get(access_token)
            .cloned()
            .ok_or(ApiError::Unauthorized)?;
        let device_id = random_id();
        let prefix = device_id.chars().take(4).collect::<String>().to_uppercase();
        let refresh_token = random_id();
        self.devices
            .lock()
            .map_err(|e| ApiError::Storage(e.to_string()))?
            .insert(
                token_hash(&refresh_token),
                Device {
                    org_id: org_id.clone(),
                },
            );
        let hostel_public_ids = self
            .hostels
            .lock()
            .map_err(|e| ApiError::Storage(e.to_string()))?
            .iter()
            .filter(|(_, hostel)| hostel.org_id == org_id)
            .map(|(id, _)| id.clone())
            .collect();
        Ok(EnrollResponse {
            device_id,
            device_prefix: prefix,
            refresh_token,
            hostel_public_ids,
        })
    }

    async fn push(
        &self,
        refresh_token: &str,
        request: PushRequest,
    ) -> Result<PushResponse, ApiError> {
        let org_id = self.device_org(refresh_token)?;
        let mut hostels = self
            .hostels
            .lock()
            .map_err(|e| ApiError::Storage(e.to_string()))?;
        let hostel = hostels
            .entry(request.hostel_public_id.clone())
            .or_insert_with(|| HostelData {
                org_id: org_id.clone(),
                ledger: Ledger::default(),
                changes: Vec::new(),
                next_cursor: 1,
            });
        if hostel.org_id != org_id {
            return Err(ApiError::Unauthorized);
        }
        let mut results = Vec::new();
        for envelope in request.commands {
            let outcome = hostel
                .ledger
                .apply(&envelope.idempotency_key, &envelope.command);
            match outcome {
                ApplyOutcome::Ack { changed, replay } => {
                    if !replay {
                        for entity in changed {
                            let cursor = hostel.next_cursor;
                            hostel.next_cursor += 1;
                            hostel.changes.push((cursor, entity));
                        }
                    }
                    results.push(CommandAck {
                        idempotency_key: envelope.idempotency_key,
                        status: "acked".to_string(),
                        message: None,
                    });
                }
                ApplyOutcome::Conflict { message } => results.push(CommandAck {
                    idempotency_key: envelope.idempotency_key,
                    status: "conflict".to_string(),
                    message: Some(message),
                }),
            }
        }
        Ok(PushResponse { results })
    }

    async fn pull(
        &self,
        refresh_token: &str,
        hostel_public_id: &str,
        cursor: i64,
    ) -> Result<PullResponse, ApiError> {
        let org_id = self.device_org(refresh_token)?;
        let hostels = self
            .hostels
            .lock()
            .map_err(|e| ApiError::Storage(e.to_string()))?;
        let Some(hostel) = hostels.get(hostel_public_id) else {
            return Ok(PullResponse {
                cursor,
                changes: Vec::new(),
                lease: None,
            });
        };
        if hostel.org_id != org_id {
            return Err(ApiError::Unauthorized);
        }
        let changes = hostel
            .changes
            .iter()
            .filter(|(at, _)| *at > cursor)
            .map(|(_, entity)| entity.clone())
            .collect();
        let name = hostel
            .ledger
            .hostel
            .as_ref()
            .map(|hostel| hostel.name.clone())
            .unwrap_or_else(|| "HostelDesk".to_string());
        Ok(PullResponse {
            cursor: hostel.next_cursor.saturating_sub(1).max(cursor),
            changes,
            lease: sign_lease(&name),
        })
    }
}

impl MemoryStore {
    fn session_for(&self, org_id: String) -> Result<AuthResponse, ApiError> {
        let access_token = random_id();
        self.sessions
            .lock()
            .map_err(|e| ApiError::Storage(e.to_string()))?
            .insert(access_token.clone(), org_id.clone());
        Ok(AuthResponse {
            access_token,
            organization_id: org_id,
        })
    }

    fn device_org(&self, refresh_token: &str) -> Result<String, ApiError> {
        self.devices
            .lock()
            .map_err(|e| ApiError::Storage(e.to_string()))?
            .get(&token_hash(refresh_token))
            .map(|device| device.org_id.clone())
            .ok_or(ApiError::Unauthorized)
    }
}

/// Router serving sign-up, enroll, push, and pull.
pub fn router(store: Arc<dyn SyncStore>) -> Router {
    Router::new()
        .route("/health", get(|| async { "ok" }))
        .route("/v1/sign-up", post(sign_up))
        .route("/v1/sign-in", post(sign_in))
        .route("/v1/devices", post(enroll))
        .route("/v1/push", post(push))
        .route("/v1/pull", post(pull))
        .with_state(store)
}

async fn sign_up(
    State(store): State<Arc<dyn SyncStore>>,
    Json(body): Json<SignUpRequest>,
) -> Result<Json<AuthResponse>, ApiError> {
    Ok(Json(store.sign_up(body).await?))
}

async fn sign_in(
    State(store): State<Arc<dyn SyncStore>>,
    Json(body): Json<SignInRequest>,
) -> Result<Json<AuthResponse>, ApiError> {
    Ok(Json(store.sign_in(body).await?))
}

async fn enroll(
    State(store): State<Arc<dyn SyncStore>>,
    headers: HeaderMap,
    Json(body): Json<EnrollRequest>,
) -> Result<Json<EnrollResponse>, ApiError> {
    let token = bearer(&headers)?;
    Ok(Json(store.enroll(token, body).await?))
}

async fn push(
    State(store): State<Arc<dyn SyncStore>>,
    headers: HeaderMap,
    Json(body): Json<PushRequest>,
) -> Result<Json<PushResponse>, ApiError> {
    let token = bearer(&headers)?;
    Ok(Json(store.push(token, body).await?))
}

async fn pull(
    State(store): State<Arc<dyn SyncStore>>,
    headers: HeaderMap,
    Json(body): Json<PullQuery>,
) -> Result<Json<PullResponse>, ApiError> {
    let token = bearer(&headers)?;
    Ok(Json(
        store
            .pull(token, &body.hostel_public_id, body.cursor)
            .await?,
    ))
}

/// Body of `POST /v1/pull`.
#[derive(Debug, Deserialize)]
pub struct PullQuery {
    /// Hostel to read.
    pub hostel_public_id: String,
    /// Last cursor the desk applied. `0` pulls everything.
    pub cursor: i64,
}

fn bearer(headers: &HeaderMap) -> Result<&str, ApiError> {
    headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .filter(|token| !token.is_empty())
        .ok_or(ApiError::Unauthorized)
}

pub(crate) fn hash_password(password: &str) -> Result<String, argon2::password_hash::Error> {
    let salt = SaltString::generate(&mut OsRng);
    Ok(Argon2::default()
        .hash_password(password.as_bytes(), &salt)?
        .to_string())
}

pub(crate) fn verify_password(
    password: &str,
    password_hash: &str,
) -> Result<bool, argon2::password_hash::Error> {
    let parsed = PasswordHash::new(password_hash)?;
    Ok(Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok())
}

pub(crate) const DUMMY_HASH: &str =
    "$argon2id$v=19$m=19456,t=2,p=1$dUTZ/yeSMQlV7a3m/Fl6dg$c3e1mge9bB8E/jomvoijodaCH200x8hfZ8jY+iKIBUM";

pub(crate) fn random_id() -> String {
    let bytes: [u8; 16] = rand_bytes();
    hex::encode(bytes)
}

fn rand_bytes() -> [u8; 16] {
    use argon2::password_hash::rand_core::RngCore;
    let mut bytes = [0u8; 16];
    OsRng.fill_bytes(&mut bytes);
    bytes
}

pub(crate) fn token_hash(token: &str) -> String {
    hex::encode(Sha256::digest(token.as_bytes()))
}

/// Sign a 14-day offline lease when a signing key is configured.
pub fn sign_lease(hostel_name: &str) -> Option<String> {
    let key = signing_key()?;
    let payload = license_payload(hostel_name);
    let json = serde_json::to_vec(&payload).ok()?;
    let signature = key.sign(&json);
    Some(format!(
        "{}:{}",
        base64::engine::general_purpose::STANDARD.encode(signature.to_bytes()),
        base64::engine::general_purpose::STANDARD.encode(json)
    ))
}

fn license_payload(hostel_name: &str) -> serde_json::Value {
    let expiry = chrono::Utc::now().date_naive() + chrono::Duration::days(14);
    serde_json::json!({
        "hostel_name": hostel_name,
        "expiry_date": expiry,
        "tier": "sync",
    })
}

fn signing_key() -> Option<SigningKey> {
    if let Ok(text) = std::env::var("HOSTELDESK_SIGNING_KEY") {
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(text.trim())
            .ok()?;
        let bytes: [u8; 32] = bytes.try_into().ok()?;
        return Some(SigningKey::from_bytes(&bytes));
    }
    #[cfg(feature = "dev-license")]
    {
        let bytes = base64::engine::general_purpose::STANDARD
            .decode("KLXBx9s0JhWFbQhtZX2IO34NajBXIy5Kor55yzOIp0E=")
            .ok()?;
        let bytes: [u8; 32] = bytes.try_into().ok()?;
        return Some(SigningKey::from_bytes(&bytes));
    }
    #[cfg(not(feature = "dev-license"))]
    None
}

/// A command the desktop sends. Re-exported so callers do not need both crates for the enum.
pub type Command = SyncCommand;

/// Serve the in-memory store on an already-bound listener. Used by tests and a local demo.
pub async fn serve_memory(listener: tokio::net::TcpListener) -> anyhow::Result<()> {
    axum::serve(listener, router(Arc::new(MemoryStore::new()))).await?;
    Ok(())
}

/// Run the HTTP server on `addr` using Postgres at `database_url`.
pub async fn serve(addr: &str, database_url: &str) -> anyhow::Result<()> {
    let store = PostgresStore::connect(database_url).await?;
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, router(Arc::new(store))).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use http_body_util::BodyExt;
    use protocol::CommandEnvelope;
    use tower::ServiceExt;

    async fn json_body(response: axum::response::Response) -> serde_json::Value {
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        serde_json::from_slice(&bytes).unwrap()
    }

    #[tokio::test]
    async fn sign_up_enroll_push_and_conflict() {
        let app = router(Arc::new(MemoryStore::new()));
        let sign_up = app
            .clone()
            .oneshot(
                axum::http::Request::builder()
                    .method("POST")
                    .uri("/v1/sign-up")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{"organization_name":"North","email":"a@b.co","password":"secret-pass"}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(sign_up.status(), StatusCode::OK);
        let auth = json_body(sign_up).await;
        let token = auth["access_token"].as_str().unwrap();

        let enrolled = app
            .clone()
            .oneshot(
                axum::http::Request::builder()
                    .method("POST")
                    .uri("/v1/devices")
                    .header("content-type", "application/json")
                    .header("authorization", format!("Bearer {token}"))
                    .body(Body::from(r#"{"device_name":"front desk"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(enrolled.status(), StatusCode::OK);
        let device = json_body(enrolled).await;
        let refresh = device["refresh_token"].as_str().unwrap();
        assert_eq!(device["device_prefix"].as_str().unwrap().len(), 4);

        let room = SyncCommand::UpsertRoom {
            public_id: "r1".into(),
            hostel_public_id: "h1".into(),
            base_revision: 0,
            room_number: "1".into(),
            room_type: "single".into(),
            capacity: 1,
            floor: None,
            semester_rate: "10".into(),
            recess_rate: "5".into(),
            status: "vacant".into(),
        };
        let check_in = SyncCommand::CheckIn {
            allocation_public_id: "a1".into(),
            hostel_public_id: "h1".into(),
            resident_public_id: "p1".into(),
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
        };
        let body = serde_json::json!({
            "hostel_public_id": "h1",
            "commands": [
                CommandEnvelope { idempotency_key: "1".into(), command: room },
                CommandEnvelope { idempotency_key: "2".into(), command: check_in.clone() },
                CommandEnvelope { idempotency_key: "3".into(), command: check_in },
            ]
        });
        let pushed = app
            .clone()
            .oneshot(
                axum::http::Request::builder()
                    .method("POST")
                    .uri("/v1/push")
                    .header("content-type", "application/json")
                    .header("authorization", format!("Bearer {refresh}"))
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(pushed.status(), StatusCode::OK);
        let payload = json_body(pushed).await;
        assert_eq!(payload["results"][0]["status"], "acked");
        assert_eq!(payload["results"][1]["status"], "acked");
        assert_eq!(payload["results"][2]["status"], "conflict");

        let pulled = app
            .oneshot(
                axum::http::Request::builder()
                    .method("POST")
                    .uri("/v1/pull")
                    .header("content-type", "application/json")
                    .header("authorization", format!("Bearer {refresh}"))
                    .body(Body::from(r#"{"hostel_public_id":"h1","cursor":0}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(pulled.status(), StatusCode::OK);
        let pull = json_body(pulled).await;
        assert!(pull["changes"].as_array().unwrap().len() >= 2);
    }
}
