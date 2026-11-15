//! Hosted sync API: sign-up, device enrollment, push, and pull.
//!
//! Built for `wasm32` this crate is the Cloudflare Worker, storing data in
//! Neon Postgres through Hyperdrive (see `pg.rs`). Built natively it serves [`MemoryStore`], which the tests
//! and the desk's two-desk sync test run against.
//!
//! Both stores go through the same [`route`] and run the same [`apply_batch`]
//! over a [`protocol::Ledger`], so the HTTP surface and the sync rules live in
//! one place. No web framework: routing is a `match` on method and path.

#[cfg(target_arch = "wasm32")]
mod pg;

use argon2::password_hash::{
    rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString,
};
use argon2::Argon2;
use protocol::{
    AckStatus, ApplyOutcome, AuthResponse, CommandAck, CommandEnvelope, EnrollRequest,
    EnrollResponse, Entity, Ledger, LicenseStatus, PullRequest, PullResponse, PushRequest,
    PushResponse, SignInRequest, SignUpRequest,
};
use serde::{de::DeserializeOwned, Serialize};
use sha2::{Digest, Sha256};

/// Most commands one push may carry. The desk sends larger outboxes in chunks.
pub const MAX_PUSH_COMMANDS: usize = 500;

/// Largest request body accepted, in bytes.
pub const MAX_BODY_BYTES: usize = 4 * 1024 * 1024;

/// Failures returned as HTTP errors.
#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    /// Wrong email or password, or a missing, expired, or revoked token.
    #[error("unauthorized")]
    Unauthorized,
    /// The body failed a check that is not a per-command sync conflict.
    #[error("{0}")]
    BadRequest(String),
    /// Too many failed sign-ins for this email; try again later.
    #[error("too many failed sign-ins, try again in 15 minutes")]
    TooManyRequests,
    /// No such route.
    #[error("not found")]
    NotFound,
    /// Storage failed. The detail is logged, not returned.
    #[error("storage error: {0}")]
    Storage(String),
}

/// An HTTP response: status code and body (JSON on success, plain text on error).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reply {
    /// HTTP status code.
    pub status: u16,
    /// Response body.
    pub body: String,
}

impl From<ApiError> for Reply {
    fn from(error: ApiError) -> Self {
        let status = match &error {
            ApiError::Unauthorized => 401,
            ApiError::BadRequest(_) => 400,
            ApiError::TooManyRequests => 429,
            ApiError::NotFound => 404,
            ApiError::Storage(detail) => {
                tracing_log(detail);
                return Reply {
                    status: 500,
                    body: "internal error".to_string(),
                };
            }
        };
        Reply {
            status,
            body: error.to_string(),
        }
    }
}

/// Errors go to the Worker's logs (`wrangler tail`, observability) or stderr natively.
fn tracing_log(detail: &str) {
    #[cfg(target_arch = "wasm32")]
    worker::console_error!("storage error: {detail}");
    #[cfg(not(target_arch = "wasm32"))]
    eprintln!("storage error: {detail}");
}

/// What [`route`] needs from a store. Crate-private: the Worker's Postgres store and
/// [`MemoryStore`] are the only implementations.
pub(crate) trait SyncStore {
    /// Create an organization and its first account.
    async fn sign_up(&self, request: SignUpRequest) -> Result<AuthResponse, ApiError>;
    /// Sign an existing account in.
    async fn sign_in(&self, request: SignInRequest) -> Result<AuthResponse, ApiError>;
    /// Enroll a desk. `access_token` comes from sign-up or sign-in.
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
    /// One page of changes after `request.cursor`.
    async fn pull(
        &self,
        refresh_token: &str,
        request: PullRequest,
    ) -> Result<PullResponse, ApiError>;
    /// The license of the account that owns `refresh_token`'s device.
    async fn license(&self, refresh_token: &str) -> Result<LicenseStatus, ApiError>;
}

/// What one push did to a ledger.
#[derive(Debug, Default)]
pub struct BatchOutcome {
    /// One result per command, in order.
    pub results: Vec<CommandAck>,
    /// Accepted rows, numbered from the `next_cursor` passed to [`apply_batch`].
    pub changes: Vec<(i64, Entity)>,
    /// Idempotency keys of commands that changed something.
    pub applied: Vec<String>,
}

/// Apply `commands` for `hostel_public_id` to `ledger`. Pure: the caller
/// persists [`BatchOutcome::changes`] and [`BatchOutcome::applied`].
pub fn apply_batch(
    ledger: &mut Ledger,
    hostel_public_id: &str,
    commands: &[CommandEnvelope],
    next_cursor: i64,
) -> BatchOutcome {
    let mut outcome = BatchOutcome::default();
    let mut cursor = next_cursor;
    for envelope in commands {
        let (status, message) = match ledger.apply_for_hostel(
            hostel_public_id,
            &envelope.idempotency_key,
            &envelope.command,
        ) {
            ApplyOutcome::Ack {
                changed,
                replay,
                warning,
            } => {
                if !replay {
                    outcome.applied.push(envelope.idempotency_key.clone());
                    for entity in changed {
                        outcome.changes.push((cursor, entity));
                        cursor += 1;
                    }
                }
                (AckStatus::Acked, warning)
            }
            ApplyOutcome::Conflict { message } => (AckStatus::Conflict, Some(message)),
        };
        outcome.results.push(CommandAck {
            idempotency_key: envelope.idempotency_key.clone(),
            status,
            message,
        });
    }
    outcome
}

/// How a store files an [`Entity`]: `(kind, public_id, lookup_key, active)`.
///
/// `lookup_key` is what the batch-scoped loads search by: room number for
/// rooms, receipt number for receipts, and room id for allocations. `active`
/// marks allocations that still hold a bed.
pub fn entity_key(entity: &Entity) -> (&'static str, &str, Option<&str>, bool) {
    match entity {
        Entity::Hostel(row) => ("hostel", &row.public_id, None, false),
        Entity::Room(row) => ("room", &row.public_id, Some(&row.room_number), false),
        Entity::Resident(row) => ("resident", &row.public_id, None, false),
        Entity::Allocation(row) => (
            "allocation",
            &row.public_id,
            Some(&row.room_public_id),
            row.status == "active",
        ),
        Entity::Payment(row) => ("payment", &row.public_id, None, false),
        Entity::Receipt(row) => ("receipt", &row.public_id, Some(&row.receipt_number), false),
        Entity::Preferences(row) => ("preferences", &row.hostel_public_id, None, false),
    }
}

/// Put loaded entities into the shape [`Ledger::from_dump`] takes.
pub fn dump_from(entities: Vec<Entity>, applied: Vec<String>) -> protocol::LedgerDump {
    let mut dump = protocol::LedgerDump {
        applied,
        ..Default::default()
    };
    for entity in entities {
        match entity {
            Entity::Hostel(row) => dump.hostel = Some(row),
            Entity::Room(row) => dump.rooms.push(row),
            Entity::Resident(row) => dump.residents.push(row),
            Entity::Allocation(row) => dump.allocations.push(row),
            Entity::Payment(row) => dump.payments.push(row),
            Entity::Receipt(row) => dump.receipts.push(row),
            Entity::Preferences(row) => dump.preferences = Some(row),
        }
    }
    dump
}

/// Trimmed, lower-cased email, so `A@B.co ` and `a@b.co` are one account.
pub fn normalize_email(email: &str) -> String {
    email.trim().to_lowercase()
}

/// Reject sign-up input the API will not store.
pub fn validate_sign_up(request: &SignUpRequest) -> Result<(), ApiError> {
    if request.organization_name.trim().is_empty() {
        return Err(ApiError::BadRequest("organization name is required".into()));
    }
    let email = normalize_email(&request.email);
    if email.len() > 254 || !email.contains('@') {
        return Err(ApiError::BadRequest("a valid email is required".into()));
    }
    if request.password.chars().count() < 8 {
        return Err(ApiError::BadRequest(
            "password must be at least 8 characters".into(),
        ));
    }
    Ok(())
}

/// Argon2id hash with a random salt.
pub fn hash_password(password: &str) -> Result<String, ApiError> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|hash| hash.to_string())
        .map_err(|e| ApiError::Storage(e.to_string()))
}

/// True when `password` matches `password_hash`. A malformed hash never matches.
pub fn verify_password(password: &str, password_hash: &str) -> bool {
    PasswordHash::new(password_hash)
        .map(|parsed| {
            Argon2::default()
                .verify_password(password.as_bytes(), &parsed)
                .is_ok()
        })
        .unwrap_or(false)
}

/// Hash verified when the email is unknown, so timing does not reveal which emails exist.
pub const DUMMY_HASH: &str =
    "$argon2id$v=19$m=19456,t=2,p=1$dUTZ/yeSMQlV7a3m/Fl6dg$c3e1mge9bB8E/jomvoijodaCH200x8hfZ8jY+iKIBUM";

/// 128-bit random hex id, used for ids and bearer tokens.
pub fn random_id() -> String {
    use argon2::password_hash::rand_core::RngCore;
    let mut bytes = [0u8; 16];
    OsRng.fill_bytes(&mut bytes);
    hex::encode(bytes)
}

/// Six upper-case hex characters for receipt numbers. Stores retry on a
/// collision within the organization.
pub fn random_device_prefix() -> String {
    random_id()[..6].to_uppercase()
}

/// Tokens are stored only as SHA-256 hashes.
pub fn token_hash(token: &str) -> String {
    hex::encode(Sha256::digest(token.as_bytes()))
}

/// Free trial every new account starts with. Paid plans are set with
/// `scripts/license-account.sh`.
pub const TRIAL_DAYS: i64 = 30;

/// Serve one request. `authorization` is the raw `Authorization` header.
pub(crate) async fn route(
    store: &impl SyncStore,
    method: &str,
    path: &str,
    authorization: Option<&str>,
    body: &[u8],
) -> Reply {
    match dispatch(store, method, path, authorization, body).await {
        Ok(body) => Reply { status: 200, body },
        Err(error) => error.into(),
    }
}

async fn dispatch(
    store: &impl SyncStore,
    method: &str,
    path: &str,
    authorization: Option<&str>,
    body: &[u8],
) -> Result<String, ApiError> {
    if body.len() > MAX_BODY_BYTES {
        return Err(ApiError::BadRequest("request body is too large".into()));
    }
    match (method, path) {
        ("GET", "/health") => Ok("ok".to_string()),
        ("POST", "/v1/sign-up") => {
            let request: SignUpRequest = parse(body)?;
            validate_sign_up(&request)?;
            json(&store.sign_up(request).await?)
        }
        ("POST", "/v1/sign-in") => json(&store.sign_in(parse(body)?).await?),
        ("POST", "/v1/devices") => {
            let token = bearer(authorization)?;
            json(&store.enroll(token, parse(body)?).await?)
        }
        ("POST", "/v1/push") => {
            let token = bearer(authorization)?;
            let request: PushRequest = parse(body)?;
            if request.commands.len() > MAX_PUSH_COMMANDS {
                return Err(ApiError::BadRequest(format!(
                    "push at most {MAX_PUSH_COMMANDS} commands at a time"
                )));
            }
            json(&store.push(token, request).await?)
        }
        ("POST", "/v1/pull") => {
            let token = bearer(authorization)?;
            json(&store.pull(token, parse(body)?).await?)
        }
        ("POST", "/v1/license") => json(&store.license(bearer(authorization)?).await?),
        _ => Err(ApiError::NotFound),
    }
}

fn parse<T: DeserializeOwned>(body: &[u8]) -> Result<T, ApiError> {
    serde_json::from_slice(body).map_err(|e| ApiError::BadRequest(format!("invalid body: {e}")))
}

fn json(value: &impl Serialize) -> Result<String, ApiError> {
    serde_json::to_string(value).map_err(|e| ApiError::Storage(e.to_string()))
}

fn bearer(authorization: Option<&str>) -> Result<&str, ApiError> {
    authorization
        .and_then(|value| value.strip_prefix("Bearer "))
        .filter(|token| !token.is_empty())
        .ok_or(ApiError::Unauthorized)
}

#[cfg(not(target_arch = "wasm32"))]
mod memory;
#[cfg(not(target_arch = "wasm32"))]
pub use memory::{serve_memory, MemoryStore};
