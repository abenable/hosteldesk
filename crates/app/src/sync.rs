//! Push the local outbox and pull server rows back into SQLite.
//!
//! The device refresh token is a file in the app data directory readable only
//! by the signed-in user (mode `0600` on Unix; the per-user profile ACL on
//! Windows), like an SSH key. It only authorizes sync for this one device and
//! can be revoked on the server.

use std::time::Duration;

use protocol::{
    AckStatus, AuthResponse, CommandEnvelope, EnrollRequest, EnrollResponse, LicenseStatus,
    PullRequest, PullResponse, PushRequest, PushResponse, SignInRequest, SignUpRequest,
};
use serde::{Deserialize, Serialize};

use crate::paths::AppPaths;
use crate::Desk;

/// The hosted sync API (Cloudflare Worker). Overridable in Settings → Cloud.
pub const DEFAULT_SERVER_URL: &str = "https://hosteldesk-sync.ableabenaitwe.workers.dev";

/// Commands per push request; the server accepts at most `server::MAX_PUSH_COMMANDS`.
const PUSH_CHUNK: usize = 200;

/// What one sync pass changed.
#[derive(Debug, Clone, Default, Serialize)]
pub struct SyncReport {
    /// Commands the server accepted.
    pub pushed: usize,
    /// Rows applied from the pull.
    pub applied: usize,
    /// Commands rejected or accepted with a warning.
    pub issues: usize,
    /// Text for a toast when changes need attention; empty otherwise.
    pub summary: String,
}

/// Cloud registration shown in Settings.
#[derive(Debug, Clone, Default, Serialize)]
pub struct CloudStatus {
    /// Server URL, once the desk has enrolled.
    pub server_url: Option<String>,
    /// Receipt prefix assigned at enroll.
    pub device_prefix: Option<String>,
    /// Last applied cursor.
    pub cursor: String,
    /// Commands still waiting to push.
    pub pending: i64,
    /// Rejected changes and warnings, newest first.
    pub issues: Vec<db::SyncIssue>,
}

/// Payload for [`Desk::connect_cloud`]. No `Debug`: it carries a password.
#[derive(Deserialize)]
pub struct ConnectCloudRequest {
    /// Sync server; blank means [`DEFAULT_SERVER_URL`].
    pub server_url: String,
    /// Account email.
    pub email: String,
    /// Account password.
    pub password: String,
    /// Create a new account (and organization) instead of signing in.
    pub create_account: bool,
    /// Organization name for a new account; ignored when signing in.
    pub organization_name: String,
}

/// Outcome of [`Desk::connect_cloud`]: the computer is signed in to the account.
#[derive(Debug, Clone, Serialize)]
pub struct ConnectResult {
    /// The account's license is usable today, so the desk is unlocked.
    pub licensed: bool,
    /// Why the desk stays locked, when `licensed` is false.
    pub lock_reason: Option<String>,
    /// The first sync failed; the background loop retries it.
    pub sync_error: Option<String>,
}

/// One background or manual sync pass, as the frontend sees it. Emitted as
/// the `sync` event after each background pass, and returned by `sync_now`.
#[derive(Debug, Clone, Serialize)]
pub struct SyncEvent {
    /// Changes that need attention, for a toast; empty otherwise.
    pub summary: String,
    /// The pass received rows from the cloud, so the window reloads its data.
    pub changed: bool,
    /// Why the pass failed; it is retried on the next tick.
    pub error: Option<String>,
    /// The account's license is usable today (a locked desk can re-boot).
    pub licensed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CloudConfig {
    server_url: String,
    device_id: String,
    device_prefix: String,
    #[serde(default)]
    hostel_public_ids: Vec<String>,
}

impl Desk {
    /// Sign this computer in to a HostelDesk account ([`Desk::enroll`]), then
    /// run one sync so an approved account unlocks right away.
    ///
    /// Errors only when connecting fails; a failed first sync is reported in
    /// [`ConnectResult::sync_error`] and retried by the background loop.
    pub async fn connect_cloud(
        &self,
        request: ConnectCloudRequest,
    ) -> Result<ConnectResult, String> {
        self.enroll(
            &request.server_url,
            &request.email,
            &request.password,
            request.create_account,
            &request.organization_name,
        )
        .await?;
        let sync_error = self.sync_once().await.err();
        let licensed = self.license_status().is_some();
        Ok(ConnectResult {
            licensed,
            lock_reason: (!licensed).then(|| {
                crate::lock_reason(self.cached_license(), chrono::Utc::now().date_naive())
            }),
            sync_error,
        })
    }

    /// One [`Desk::sync_once`] pass reported as a [`SyncEvent`]. Never fails:
    /// an error is part of the event.
    pub async fn sync_now(&self) -> SyncEvent {
        let (summary, changed, error) = match self.sync_once().await {
            Ok(report) => (report.summary, report.applied > 0, None),
            Err(error) => (String::new(), false, Some(error)),
        };
        SyncEvent {
            summary,
            changed,
            error,
            licensed: self.license_status().is_some(),
        }
    }

    /// Create or sign in to an organization, enroll this computer, and store the token.
    ///
    /// The first desk of an organization uploads its existing hostel. A desk
    /// joining an organization that already has a hostel must be a fresh
    /// install; merging two different hostels is refused.
    ///
    /// The account is the license, and the server is the only judge of it:
    /// after enrolling, the desk asks `/v1/license` and unlocks while the
    /// account's plan runs.
    pub(crate) async fn enroll(
        &self,
        server_url: &str,
        email: &str,
        password: &str,
        create_account: bool,
        organization_name: &str,
    ) -> Result<(), String> {
        let server_url = normalize_server_url(server_url)?;
        let client = http_client()?;
        let auth: AuthResponse = if create_account {
            let body = SignUpRequest {
                organization_name: organization_name.trim().to_string(),
                email: email.trim().to_string(),
                password: password.to_string(),
            };
            post_json(&client, &format!("{server_url}/v1/sign-up"), None, &body).await?
        } else {
            let body = SignInRequest {
                email: email.trim().to_string(),
                password: password.to_string(),
            };
            post_json(&client, &format!("{server_url}/v1/sign-in"), None, &body).await?
        };
        let enrolled: EnrollResponse = post_json(
            &client,
            &format!("{server_url}/v1/devices"),
            Some(&auth.access_token),
            &EnrollRequest {
                device_name: device_name(),
            },
        )
        .await?;

        let local = self.db.primary_hostel_id().await.map_err(err)?;
        let mut snapshot = enrolled.hostel_public_ids.is_empty();
        if let Some(hostel_id) = local {
            let public_id = self.db.hostel_public_id(hostel_id).await.map_err(err)?;
            let known = public_id
                .as_ref()
                .is_some_and(|id| enrolled.hostel_public_ids.contains(id));
            if !snapshot && !known {
                return Err(
                    "This organization already has a hostel in the cloud, and this \
                     computer holds a different one. To add this computer to that \
                     organization, install HostelDesk fresh and sign in before setting \
                     up a hostel. To put this hostel in the cloud, create a new \
                     organization instead."
                        .to_string(),
                );
            }
            snapshot = snapshot && !known;
        }

        store_token(&self.paths, &enrolled.refresh_token)?;

        write_config(
            &self.paths,
            &CloudConfig {
                server_url: server_url.clone(),
                device_id: enrolled.device_id.clone(),
                device_prefix: enrolled.device_prefix.clone(),
                hostel_public_ids: enrolled.hostel_public_ids,
            },
        )?;
        if let Some(hostel_id) = local {
            self.db
                .link_device(
                    hostel_id,
                    &server_url,
                    &enrolled.device_id,
                    &enrolled.device_prefix,
                    snapshot,
                )
                .await
                .map_err(err)?;
        }
        self.refresh_license(&client, &server_url, &enrolled.refresh_token)
            .await
    }

    /// Ask the server for this account's license and cache the answer.
    ///
    /// A network failure keeps the cached answer (the desk keeps working for
    /// up to [`crate::OFFLINE_GRACE_DAYS`]); a rejected device token clears it.
    async fn refresh_license(
        &self,
        client: &reqwest::Client,
        server_url: &str,
        token: &str,
    ) -> Result<(), String> {
        let response = client
            .post(format!("{server_url}/v1/license"))
            .bearer_auth(token)
            .json(&serde_json::json!({}))
            .send()
            .await
            .map_err(err)?;
        if response.status() == reqwest::StatusCode::UNAUTHORIZED {
            self.license.clear();
            let _ = std::fs::remove_file(self.paths.license_path());
            return Err("this computer is no longer signed in to its account".to_string());
        }
        if !response.status().is_success() {
            return Err(format!("license check failed: {}", response.status()));
        }
        let status: LicenseStatus = response.json().await.map_err(err)?;
        self.store_license(crate::AccountLicense {
            account_name: status.account_name,
            plan: status.plan,
            expires_on: status.expires_on,
            checked_on: chrono::Utc::now().date_naive(),
            approved: status.approved,
        })
    }

    /// True when this computer has enrolled and still has a refresh token.
    pub fn cloud_enrolled(&self) -> bool {
        read_config(&self.paths).is_some() && read_token(&self.paths).is_some()
    }

    /// Local changes are waiting to send. The background loop checks this
    /// every few seconds so a payment or a new room reaches the cloud right
    /// away; a database error reads as "no" and the next full pass sends it.
    pub async fn has_pending_sync(&self) -> bool {
        match self.db.primary_hostel_id().await {
            Ok(Some(hostel_id)) => self
                .db
                .pending_sync_count(hostel_id)
                .await
                .is_ok_and(|count| count > 0),
            _ => false,
        }
    }

    /// Push pending commands, then pull every new page. No-op when not enrolled.
    ///
    /// Passes run one at a time; a second caller waits for the first.
    pub async fn sync_once(&self) -> Result<SyncReport, String> {
        let _one_at_a_time = self.sync_gate.lock().await;
        let (Some(config), Some(token)) = (read_config(&self.paths), read_token(&self.paths))
        else {
            return Ok(SyncReport::default());
        };
        let client = http_client()?;
        let mut report = SyncReport::default();
        self.refresh_license(&client, &config.server_url, &token)
            .await?;
        if !self.license.get().is_some_and(|license| license.approved) {
            // The server refuses to sync a pending account; nothing to do yet.
            return Ok(report);
        }

        match self.db.primary_hostel_id().await.map_err(err)? {
            Some(hostel_id) => {
                let public_id = self
                    .db
                    .hostel_public_id(hostel_id)
                    .await
                    .map_err(err)?
                    .ok_or("hostel has no public id")?;
                // Links a hostel created after enrolling (onboarding after
                // cloud sign-in); a no-op once linked to this device.
                let snapshot = !config.hostel_public_ids.contains(&public_id);
                self.db
                    .link_device(
                        hostel_id,
                        &config.server_url,
                        &config.device_id,
                        &config.device_prefix,
                        snapshot,
                    )
                    .await
                    .map_err(err)?;
                self.push_pending(&client, &config, &token, hostel_id, &public_id, &mut report)
                    .await?;
                self.pull_all(&client, &config, &token, &public_id, &mut report)
                    .await?;
            }
            None => {
                // A fresh desk joining an organization: download its hostel.
                let Some(public_id) = config.hostel_public_ids.first() else {
                    return Ok(report);
                };
                if let Some(hostel_id) = self
                    .pull_all(&client, &config, &token, public_id, &mut report)
                    .await?
                {
                    self.db
                        .link_device(
                            hostel_id,
                            &config.server_url,
                            &config.device_id,
                            &config.device_prefix,
                            false,
                        )
                        .await
                        .map_err(err)?;
                }
            }
        }

        // Sync is automatic and the sidebar shows "Synced"; only a problem is
        // worth a toast. (`applied` also counts this desk's own changes coming
        // back from the server, so it cannot tell "another computer" apart.)
        if report.issues > 0 {
            let changes = if report.issues == 1 {
                "change needs"
            } else {
                "changes need"
            };
            report.summary = format!(
                "{} synced {changes} attention (Settings → Cloud).",
                report.issues
            );
        }
        Ok(report)
    }

    async fn push_pending(
        &self,
        client: &reqwest::Client,
        config: &CloudConfig,
        token: &str,
        hostel_id: i64,
        hostel_public_id: &str,
        report: &mut SyncReport,
    ) -> Result<(), String> {
        let pending = self.db.pending_sync(hostel_id).await.map_err(err)?;
        for chunk in pending.chunks(PUSH_CHUNK) {
            let request = PushRequest {
                hostel_public_id: hostel_public_id.to_string(),
                commands: chunk
                    .iter()
                    .map(|row| CommandEnvelope {
                        idempotency_key: row.idempotency_key.clone(),
                        command: row.command.clone(),
                    })
                    .collect(),
            };
            let pushed: PushResponse = post_json(
                client,
                &format!("{}/v1/push", config.server_url),
                Some(token),
                &request,
            )
            .await?;
            if pushed.results.len() != chunk.len() {
                return Err("server ack count did not match the outbox".to_string());
            }
            let acks: Vec<db::OutboxAck> = chunk
                .iter()
                .zip(pushed.results)
                .map(|(row, ack)| (row.id, ack.status, ack.message))
                .collect();
            for (_, status, message) in &acks {
                if *status == AckStatus::Acked {
                    report.pushed += 1;
                }
                if message.is_some() {
                    report.issues += 1;
                }
            }
            self.db.record_acks(&acks).await.map_err(err)?;
        }
        Ok(())
    }

    /// Pull pages until the server has nothing more. Returns the local id of
    /// the hostel once it exists on this desk.
    async fn pull_all(
        &self,
        client: &reqwest::Client,
        config: &CloudConfig,
        token: &str,
        hostel_public_id: &str,
        report: &mut SyncReport,
    ) -> Result<Option<i64>, String> {
        let mut hostel_id = match self.db.primary_hostel_id().await.map_err(err)? {
            Some(id)
                if self.db.hostel_public_id(id).await.map_err(err)?.as_deref()
                    == Some(hostel_public_id) =>
            {
                Some(id)
            }
            _ => None,
        };
        let mut cursor = match hostel_id {
            Some(id) => self
                .db
                .sync_state(id)
                .await
                .map_err(err)?
                .and_then(|state| state.cursor.parse::<i64>().ok())
                .unwrap_or(0),
            None => 0,
        };
        loop {
            let pulled: PullResponse = post_json(
                client,
                &format!("{}/v1/pull", config.server_url),
                Some(token),
                &PullRequest {
                    hostel_public_id: hostel_public_id.to_string(),
                    cursor,
                },
            )
            .await?;
            hostel_id = self
                .db
                .apply_remote_page(hostel_public_id, &pulled.changes, pulled.cursor)
                .await
                .map_err(err)?
                .or(hostel_id);
            report.applied += pulled.changes.len();
            cursor = pulled.cursor;
            if !pulled.has_more {
                return Ok(hostel_id);
            }
        }
    }

    /// Registration, outbox count, and issues for Settings.
    pub async fn cloud_status(&self) -> Result<CloudStatus, String> {
        let config = read_config(&self.paths);
        let mut status = CloudStatus {
            server_url: config.as_ref().map(|config| config.server_url.clone()),
            device_prefix: config.as_ref().map(|config| config.device_prefix.clone()),
            cursor: "0".to_string(),
            ..CloudStatus::default()
        };
        let Some(hostel_id) = self.db.primary_hostel_id().await.map_err(err)? else {
            return Ok(status);
        };
        if let Some(state) = self.db.sync_state(hostel_id).await.map_err(err)? {
            status.cursor = state.cursor;
            status.server_url = state.server_url.or(status.server_url);
            status.device_prefix = state.device_prefix.or(status.device_prefix);
        }
        status.pending = self.db.pending_sync_count(hostel_id).await.map_err(err)?;
        status.issues = self.db.sync_issues(hostel_id).await.map_err(err)?;
        Ok(status)
    }
}

fn err(error: impl std::fmt::Display) -> String {
    error.to_string()
}

/// Trim the address, fall back to [`DEFAULT_SERVER_URL`], and require HTTPS
/// (plain HTTP only to this machine in debug builds, for tests).
fn normalize_server_url(server_url: &str) -> Result<String, String> {
    let url = server_url.trim().trim_end_matches('/');
    let url = if url.is_empty() {
        DEFAULT_SERVER_URL
    } else {
        url
    };
    let local_http = cfg!(debug_assertions)
        && (url.starts_with("http://127.0.0.1") || url.starts_with("http://localhost"));
    if url.starts_with("https://") || local_http {
        Ok(url.to_string())
    } else {
        Err("The sync server address must start with https://".to_string())
    }
}

fn device_name() -> String {
    std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .unwrap_or_else(|_| "HostelDesk".to_string())
}

fn http_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .build()
        .map_err(err)
}

async fn post_json<T: for<'de> Deserialize<'de>>(
    client: &reqwest::Client,
    url: &str,
    bearer: Option<&str>,
    body: &impl Serialize,
) -> Result<T, String> {
    let mut request = client.post(url).json(body);
    if let Some(token) = bearer {
        request = request.bearer_auth(token);
    }
    let response = request.send().await.map_err(err)?;
    let status = response.status();
    if !status.is_success() {
        let text = response.text().await.unwrap_or_default();
        return Err(format!("server returned {status}: {text}"));
    }
    response.json().await.map_err(err)
}

fn write_config(paths: &AppPaths, config: &CloudConfig) -> Result<(), String> {
    std::fs::create_dir_all(&paths.data_dir).map_err(err)?;
    let text = serde_json::to_string_pretty(config).map_err(err)?;
    std::fs::write(paths.cloud_config_path(), text).map_err(err)
}

fn read_config(paths: &AppPaths) -> Option<CloudConfig> {
    let text = std::fs::read_to_string(paths.cloud_config_path()).ok()?;
    serde_json::from_str(&text).ok()
}

fn store_token(paths: &AppPaths, token: &str) -> Result<(), String> {
    write_token_file(paths, token)
}

fn read_token(paths: &AppPaths) -> Option<String> {
    std::fs::read_to_string(paths.device_token_path())
        .ok()
        .map(|token| token.trim().to_string())
        .filter(|token| !token.is_empty())
}

fn write_token_file(paths: &AppPaths, token: &str) -> Result<(), String> {
    use std::io::Write;
    std::fs::create_dir_all(&paths.data_dir).map_err(err)?;
    let path = paths.device_token_path();
    let mut options = std::fs::OpenOptions::new();
    options.create(true).write(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(&path).map_err(err)?;
    // `mode` applies only on create; tighten a file that already existed.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).map_err(err)?;
    }
    file.write_all(token.as_bytes()).map_err(err)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{AuthState, LicenseState};
    use db::{DbPool, NewCheckIn, NewHostel, NewPayment, NewRoom, NewRoomClass};
    use std::sync::Arc;

    async fn memory_server() -> (String, Arc<server::MemoryStore>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind");
        let addr = listener.local_addr().expect("addr");
        let store = Arc::new(server::MemoryStore::new());
        let served = store.clone();
        tokio::spawn(async move {
            let _ = server::serve_memory(listener, served).await;
        });
        (format!("http://{addr}"), store)
    }

    async fn desk_at(root: &std::path::Path) -> Desk {
        let paths = AppPaths::for_test(root);
        std::fs::create_dir_all(&paths.data_dir).expect("data dir");
        let db = DbPool::open(&paths.database_url()).await.expect("db");
        Desk::from_parts(db, AuthState::new(), LicenseState::default(), paths)
    }

    fn stay(hostel_id: i64, room_id: i64, name: &str) -> NewCheckIn {
        NewCheckIn {
            hostel_id,
            room_id,
            resident_id: None,
            full_name: Some(name.into()),
            phone: None,
            email: None,
            id_number: None,
            next_of_kin: None,
            next_of_kin_phone: None,
            photo_path: None,
            rate_agreed: "100".into(),
            term: "semester_1".into(),
            academic_year: "2026/2027".into(),
        }
    }

    async fn scalar(desk: &Desk, sql: &'static str) -> String {
        sqlx::query_scalar(sql)
            .fetch_one(&desk.db.inner)
            .await
            .expect(sql)
    }

    /// A local change is waiting to send until the next pass, which sends it
    /// without a toast: only received changes and issues are worth one.
    #[tokio::test]
    async fn a_local_change_waits_to_send_and_sending_it_is_silent() {
        let (url, store) = memory_server().await;
        let root = tempfile::tempdir().expect("temp");
        let desk = desk_at(root.path()).await;
        let hostel = desk
            .db
            .create_hostel(NewHostel {
                name: "North".into(),
                address: None,
                phone: None,
                currency: Some("UGX".into()),
                branding_logo_path: None,
            })
            .await
            .expect("hostel");
        let class_id = desk
            .db
            .add_room_class(NewRoomClass {
                hostel_id: hostel.id,
                floor: None,
                room_type: "single".into(),
                semester_rate: "100".into(),
                recess_rate: "40".into(),
            })
            .await
            .expect("class");
        assert!(
            !desk.has_pending_sync().await,
            "nothing waits on a desk that never enrolled"
        );
        desk.enroll(&url, "a@b.co", "secret-pass", true, "North")
            .await
            .expect("sign up");
        store.approve("a@b.co").expect("approve");
        assert!(desk.has_pending_sync().await, "the hostel waits to upload");
        assert!(desk.sync_now().await.error.is_none());
        assert!(!desk.has_pending_sync().await, "all sent");

        desk.db
            .add_room(NewRoom {
                hostel_id: hostel.id,
                room_number: "7".into(),
                class_id,
            })
            .await
            .expect("room");
        assert!(desk.has_pending_sync().await, "the new room waits to send");
        let event = desk.sync_now().await;
        assert_eq!(
            (event.summary.as_str(), event.error.as_deref()),
            ("", None),
            "{event:?}"
        );
        assert!(!desk.has_pending_sync().await, "the new room was sent");
    }

    /// The upgrade path end to end: a desk with v1-style history enrolls and
    /// uploads it, a fresh desk joins and receives it with original dates and
    /// receipt numbers, and an overbooked bed is flagged instead of undone.
    #[tokio::test]
    async fn existing_hostel_uploads_fresh_desk_joins_and_overbooking_is_flagged() {
        let (url, store) = memory_server().await;
        let root_a = tempfile::tempdir().expect("temp a");
        let desk_a = desk_at(root_a.path()).await;
        let hostel = desk_a
            .db
            .create_hostel(NewHostel {
                name: "North".into(),
                address: None,
                phone: None,
                currency: Some("UGX".into()),
                branding_logo_path: None,
            })
            .await
            .expect("hostel");
        let class_id = desk_a
            .db
            .add_room_class(NewRoomClass {
                hostel_id: hostel.id,
                floor: Some(1),
                room_type: "single".into(),
                semester_rate: "100".into(),
                recess_rate: "40".into(),
            })
            .await
            .expect("class");
        let room_id = desk_a
            .db
            .add_room(NewRoom {
                hostel_id: hostel.id,
                room_number: "1".into(),
                class_id,
            })
            .await
            .expect("room");
        let first_stay = desk_a
            .db
            .check_in(stay(hostel.id, room_id, "Ada"))
            .await
            .expect("check in");
        let resident_id: i64 = sqlx::query_scalar("SELECT resident_id FROM allocations")
            .fetch_one(&desk_a.db.inner)
            .await
            .expect("resident");
        desk_a
            .db
            .record_payment_and_receipt(NewPayment {
                hostel_id: hostel.id,
                allocation_id: first_stay,
                resident_id,
                amount: "60".into(),
                payment_method: "cash".into(),
                period_start: None,
                period_end: None,
                notes: None,
            })
            .await
            .expect("pay");
        sqlx::query("UPDATE payments SET payment_date = '2026-09-01T08:00:00Z'")
            .execute(&desk_a.db.inner)
            .await
            .expect("backdate");
        desk_a
            .db
            .check_out(hostel.id, first_stay)
            .await
            .expect("out");
        assert_eq!(
            desk_a.db.pending_sync_count(hostel.id).await.unwrap(),
            0,
            "nothing queues before the desk enrolls"
        );

        desk_a
            .enroll(&url, "a@b.co", "secret-pass", true, "North")
            .await
            .expect("sign up");
        assert!(
            desk_a.license_status().is_none(),
            "a new account waits for the owner's approval"
        );
        store.approve("a@b.co").expect("approve");
        let report = desk_a.sync_once().await.expect("sync a");
        assert_eq!(report.issues, 0, "{report:?}");
        assert!(desk_a.license_status().is_some(), "unlocked once approved");
        assert!(report.pushed >= 5, "{report:?}");
        assert_eq!(desk_a.db.pending_sync_count(hostel.id).await.unwrap(), 0);
        assert_eq!(
            desk_a.license_status().map(|license| license.plan),
            Some("trial".to_string()),
            "an approved account unlocks on its trial"
        );

        // A desk that already has its own hostel may not join this organization.
        let root_c = tempfile::tempdir().expect("temp c");
        let desk_c = desk_at(root_c.path()).await;
        desk_c
            .db
            .create_hostel(NewHostel {
                name: "Other".into(),
                address: None,
                phone: None,
                currency: None,
                branding_logo_path: None,
            })
            .await
            .expect("other hostel");
        let refused = desk_c
            .enroll(&url, "a@b.co", "secret-pass", false, "")
            .await
            .expect_err("different hostel must be refused");
        assert!(refused.contains("fresh"), "{refused}");

        let root_b = tempfile::tempdir().expect("temp b");
        let desk_b = desk_at(root_b.path()).await;
        desk_b
            .enroll(&url, "a@b.co", "secret-pass", false, "")
            .await
            .expect("sign in");
        desk_b.sync_once().await.expect("sync b");
        assert_eq!(scalar(&desk_b, "SELECT name FROM hostels").await, "North");
        assert_eq!(
            scalar(&desk_b, "SELECT payment_date FROM payments").await,
            "2026-09-01T08:00:00Z"
        );
        assert_eq!(
            scalar(&desk_b, "SELECT receipt_number FROM receipts").await,
            scalar(&desk_a, "SELECT receipt_number FROM receipts").await
        );
        assert_eq!(
            scalar(&desk_b, "SELECT status FROM allocations").await,
            "checked_out"
        );

        // Both desks check someone into the same room while offline: rooms
        // have no bed count, so both stays stand and nothing is flagged.
        let hostel_b = desk_b.db.primary_hostel_id().await.unwrap().unwrap();
        let room_b = desk_b.db.get_rooms(hostel_b).await.unwrap()[0].id;
        desk_a
            .db
            .check_in(stay(hostel.id, room_id, "Grace"))
            .await
            .expect("a books");
        desk_b
            .db
            .check_in(stay(hostel_b, room_b, "Linus"))
            .await
            .expect("b books");
        assert_eq!(desk_a.sync_once().await.expect("a").issues, 0);
        let second = desk_b.sync_once().await.expect("b");
        assert_eq!(second.issues, 0, "{second:?}");
        assert_eq!(
            desk_b.db.get_rooms(hostel_b).await.unwrap()[0].floor,
            Some(1),
            "the room's class came across with it"
        );
        desk_a.sync_once().await.expect("a catches up");
        for desk in [&desk_a, &desk_b] {
            assert_eq!(
                scalar(
                    desk,
                    "SELECT CAST(COUNT(*) AS TEXT) FROM allocations WHERE status = 'active'"
                )
                .await,
                "2",
                "neither stay is rolled back"
            );
        }
    }

    /// Licensing is on the account: a fresh computer unlocks by signing in,
    /// and only while the account's license runs.
    #[tokio::test]
    async fn signing_in_unlocks_only_while_the_account_is_licensed() {
        let (url, store) = memory_server().await;
        let root = tempfile::tempdir().expect("temp");
        let desk = desk_at(root.path()).await;
        assert!(desk.license_status().is_none());
        desk.enroll(&url, "owner@hostel.co", "secret-pass", true, "Sunrise")
            .await
            .expect("sign up");
        let pending = desk.cached_license().expect("the server answered");
        assert!(!pending.approved && desk.license_status().is_none());
        store.approve("owner@hostel.co").expect("approve");
        desk.sync_once().await.expect("check again");
        let license = desk
            .license_status()
            .expect("unlocked once approved, before any hostel");
        assert_eq!(license.account_name, "Sunrise");
        assert_eq!(license.checked_on, chrono::Utc::now().date_naive());

        // The account's license ends: a computer signing in now stays locked.
        let auth: protocol::AuthResponse = reqwest::Client::new()
            .post(format!("{url}/v1/sign-in"))
            .json(&serde_json::json!({"email":"owner@hostel.co","password":"secret-pass"}))
            .send()
            .await
            .expect("sign in")
            .json()
            .await
            .expect("auth");
        let yesterday = chrono::Utc::now().date_naive() - chrono::Duration::days(1);
        store
            .set_license(&auth.organization_id, "trial", yesterday)
            .expect("expire");
        let root_b = tempfile::tempdir().expect("temp b");
        let desk_b = desk_at(root_b.path()).await;
        desk_b
            .enroll(&url, "owner@hostel.co", "secret-pass", false, "")
            .await
            .expect("sign in");
        assert!(desk_b.license_status().is_none());
    }

    /// The Lock screen flow: connecting says why a new account stays locked,
    /// and the background sync reports the unlock once the owner approves.
    #[tokio::test]
    async fn connect_cloud_explains_the_lock_and_sync_now_reports_the_unlock() {
        let (url, store) = memory_server().await;
        let root = tempfile::tempdir().expect("temp");
        let desk = desk_at(root.path()).await;
        let request = |password: &str| ConnectCloudRequest {
            server_url: url.clone(),
            email: "owner@hostel.co".into(),
            password: password.into(),
            create_account: true,
            organization_name: "Sunrise".into(),
        };

        let idle = desk.sync_now().await;
        assert_eq!(
            (idle.summary.as_str(), idle.error, idle.licensed),
            ("", None, false),
            "a desk that never enrolled has nothing to sync"
        );

        let connected = desk
            .connect_cloud(request("secret-pass"))
            .await
            .expect("connect");
        assert!(!connected.licensed);
        assert_eq!(connected.sync_error, None);
        assert!(connected
            .lock_reason
            .as_deref()
            .is_some_and(|reason| reason.contains("waiting for approval")));
        let boot = desk.boot_state().await.expect("boot");
        assert_eq!(boot.gate, crate::Gate::Lock);
        assert!(boot.enrolled);
        assert_eq!(boot.lock_reason, connected.lock_reason);

        store.approve("owner@hostel.co").expect("approve");
        let event = desk.sync_now().await;
        assert!(event.licensed && event.error.is_none(), "{event:?}");
        assert_eq!(
            desk.boot_state().await.expect("boot").gate,
            crate::Gate::Onboarding
        );

        let other = tempfile::tempdir().expect("temp b");
        let refused = desk_at(other.path())
            .await
            .connect_cloud(ConnectCloudRequest {
                create_account: false,
                ..request("wrong-pass")
            })
            .await;
        assert!(refused.is_err(), "a wrong password does not connect");
    }

    #[test]
    fn server_address_must_be_https() {
        assert!(normalize_server_url("http://sync.example.com").is_err());
        assert_eq!(
            normalize_server_url(" https://sync.example.com/ ").unwrap(),
            "https://sync.example.com"
        );
        assert_eq!(normalize_server_url("").unwrap(), DEFAULT_SERVER_URL);
    }
}
