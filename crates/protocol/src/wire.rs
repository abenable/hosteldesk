//! HTTP request and response bodies shared by the desk and the server.

use crate::{CommandEnvelope, Entity};
use serde::{Deserialize, Serialize};

/// Most changes one `POST /v1/pull` returns. The desk pulls again while
/// [`PullResponse::has_more`] is set.
pub const PULL_PAGE_SIZE: i64 = 1000;

/// Body of `POST /v1/sign-up`: a new organization and its first account.
///
/// No `Debug`: it carries a password.
#[derive(Clone, Serialize, Deserialize)]
pub struct SignUpRequest {
    /// Customer name.
    pub organization_name: String,
    /// Login email.
    pub email: String,
    /// Plain password. The server stores only an Argon2 hash.
    pub password: String,
}

/// Body of `POST /v1/sign-in`.
///
/// No `Debug`: it carries a password.
#[derive(Clone, Serialize, Deserialize)]
pub struct SignInRequest {
    /// Login email.
    pub email: String,
    /// Plain password.
    pub password: String,
}

/// Session returned by sign-up and sign-in. The token authorizes device enrollment only.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AuthResponse {
    /// Bearer token for `POST /v1/devices`. Expires after 30 days.
    pub access_token: String,
    /// Organization id.
    pub organization_id: String,
}

/// Body of `POST /v1/devices`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EnrollRequest {
    /// Human-readable device name.
    pub device_name: String,
}

/// Credentials the desk stores after enrolling.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EnrollResponse {
    /// Stable device id.
    pub device_id: String,
    /// Prefix unique within the organization, used in receipt numbers (`AB12CD-0007`).
    pub device_prefix: String,
    /// Bearer token for push and pull.
    pub refresh_token: String,
    /// Hostels this organization already has on the server. Empty for the first desk.
    #[serde(default)]
    pub hostel_public_ids: Vec<String>,
}

/// Body of `POST /v1/license`: the account's license, as the server holds it.
///
/// The server is the only authority. A desk is usable while `expires_on` has
/// not passed and it heard this from the server recently (see the desk's
/// offline grace period).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LicenseStatus {
    /// Organization name, shown on the desk.
    pub account_name: String,
    /// `trial`, `semester`, `1-year`, `3-year`, or `5-year`.
    pub plan: String,
    /// Last day the license is valid.
    pub expires_on: chrono::NaiveDate,
    /// The owner has approved this account. Until then the desk stays locked
    /// and the server refuses to sync its data.
    pub approved: bool,
}

/// Body of `POST /v1/push`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PushRequest {
    /// Hostel every command must belong to.
    pub hostel_public_id: String,
    /// Commands in the order they happened on the desk.
    pub commands: Vec<CommandEnvelope>,
}

/// Whether the server took a command.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AckStatus {
    /// Accepted. [`CommandAck::message`] may still carry a warning for staff.
    Acked,
    /// Rejected. [`CommandAck::message`] says why.
    Conflict,
}

/// Per-command result of a push.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CommandAck {
    /// The key the desk sent.
    pub idempotency_key: String,
    /// Accepted or rejected.
    pub status: AckStatus,
    /// Conflict reason, or a warning on an accepted command.
    pub message: Option<String>,
}

/// Result of a push: one entry per command, in request order.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PushResponse {
    /// Per-command results.
    pub results: Vec<CommandAck>,
}

/// Body of `POST /v1/pull`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PullRequest {
    /// Hostel to read.
    pub hostel_public_id: String,
    /// Last cursor the desk applied. `0` reads from the start.
    pub cursor: i64,
}

/// One page of changes after the requested cursor.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PullResponse {
    /// Cursor of the last change in `changes` (the request cursor when empty).
    pub cursor: i64,
    /// Rows in the order the server accepted them.
    pub changes: Vec<Entity>,
    /// More changes exist after `cursor`; pull again.
    #[serde(default)]
    pub has_more: bool,
}
