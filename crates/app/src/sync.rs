//! Push the local outbox and pull server rows back into SQLite.
//!
//! The refresh token lives in the OS keyring. A mode `0600` file under the
//! app data directory is the fallback when the keyring is not available.

use std::time::Duration;

use db::DbPool;
use protocol::{CommandEnvelope, Entity, SyncCommand};
use serde::{Deserialize, Serialize};

use crate::paths::AppPaths;
use crate::Desk;

const KEYRING_SERVICE: &str = "com.hosteldesk.app";
const KEYRING_USER: &str = "device-refresh";

/// What one sync pass changed.
#[derive(Debug, Clone, Default)]
pub struct SyncReport {
    /// Commands the server accepted.
    pub pushed: usize,
    /// Rows applied from the pull.
    pub applied: usize,
    /// Commands the server rejected.
    pub conflicts: usize,
    /// Short text for the status bar. Empty when nothing happened.
    pub summary: String,
}

/// Cloud registration shown in Settings.
#[derive(Debug, Clone, Default)]
pub struct CloudStatus {
    /// Server URL, once the desk has enrolled.
    pub server_url: Option<String>,
    /// Receipt prefix assigned at enroll.
    pub device_prefix: Option<String>,
    /// Last applied cursor.
    pub cursor: String,
    /// Commands still waiting to push.
    pub pending: i64,
    /// Rejected commands, newest last.
    pub conflicts: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CloudConfig {
    server_url: String,
    device_id: String,
    device_prefix: String,
    #[serde(default)]
    hostel_public_ids: Vec<String>,
}

#[derive(Deserialize)]
struct AuthBody {
    access_token: String,
}

#[derive(Deserialize)]
struct EnrollBody {
    device_id: String,
    device_prefix: String,
    refresh_token: String,
    #[serde(default)]
    hostel_public_ids: Vec<String>,
}

#[derive(Deserialize)]
struct PushBody {
    results: Vec<PushAck>,
}

#[derive(Deserialize)]
struct PushAck {
    status: String,
    message: Option<String>,
}

#[derive(Deserialize)]
struct PullBody {
    cursor: i64,
    changes: Vec<Entity>,
    lease: Option<String>,
}

impl Desk {
    /// Create or sign in to an organization, enroll this computer, and store the token.
    pub async fn connect_cloud(
        &self,
        server_url: &str,
        email: &str,
        password: &str,
        create_account: bool,
        organization_name: &str,
    ) -> Result<(), String> {
        let server_url = server_url.trim().trim_end_matches('/').to_string();
        if server_url.is_empty() {
            return Err("server address is required".to_string());
        }
        let client = http_client()?;
        let auth: AuthBody = if create_account {
            post_json(
                &client,
                &format!("{server_url}/v1/sign-up"),
                None,
                &serde_json::json!({
                    "organization_name": organization_name,
                    "email": email,
                    "password": password,
                }),
            )
            .await?
        } else {
            post_json(
                &client,
                &format!("{server_url}/v1/sign-in"),
                None,
                &serde_json::json!({ "email": email, "password": password }),
            )
            .await?
        };
        let enrolled: EnrollBody = post_json(
            &client,
            &format!("{server_url}/v1/devices"),
            Some(&auth.access_token),
            &serde_json::json!({ "device_name": "HostelDesk" }),
        )
        .await?;
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
        if let Some(hostel_id) = self
            .db
            .primary_hostel_id()
            .await
            .map_err(|e| e.to_string())?
        {
            self.db
                .save_device(
                    hostel_id,
                    &server_url,
                    &enrolled.device_id,
                    &enrolled.device_prefix,
                )
                .await
                .map_err(|e| e.to_string())?;
        }
        self.grant_online_access();
        Ok(())
    }

    /// True when this computer has enrolled and still has a refresh token.
    pub fn cloud_enrolled(&self) -> bool {
        read_config(&self.paths).is_some() && read_token(&self.paths).is_some()
    }

    /// Push pending commands, then pull and apply. No-ops when the desk is not enrolled.
    pub async fn sync_once(&self) -> Result<SyncReport, String> {
        let Some(config) = read_config(&self.paths) else {
            return Ok(SyncReport::default());
        };
        let Some(token) = read_token(&self.paths) else {
            return Ok(SyncReport::default());
        };
        let client = http_client()?;
        let mut hostel_id = self
            .db
            .primary_hostel_id()
            .await
            .map_err(|e| e.to_string())?;
        let mut cursor = if let Some(id) = hostel_id {
            self.db
                .save_device(
                    id,
                    &config.server_url,
                    &config.device_id,
                    &config.device_prefix,
                )
                .await
                .map_err(|e| e.to_string())?;
            self.db
                .sync_state(id)
                .await
                .map_err(|e| e.to_string())?
                .and_then(|row| row.cursor.parse::<i64>().ok())
                .unwrap_or(0)
        } else {
            0
        };
        let mut report = SyncReport::default();

        if hostel_id.is_none() && config.hostel_public_ids.is_empty() {
            return Ok(report);
        }
        if hostel_id.is_none() || cursor == 0 {
            cursor = pull_apply(self, &client, &config, &token, cursor, &mut report).await?;
            hostel_id = self
                .db
                .primary_hostel_id()
                .await
                .map_err(|e| e.to_string())?;
        }
        let Some(hostel_id) = hostel_id else {
            return Ok(report);
        };
        self.db
            .save_device(
                hostel_id,
                &config.server_url,
                &config.device_id,
                &config.device_prefix,
            )
            .await
            .map_err(|e| e.to_string())?;

        let pending = self
            .db
            .pending_sync(hostel_id)
            .await
            .map_err(|e| e.to_string())?;
        if !pending.is_empty() {
            let hostel_public_id = self
                .db
                .hostel_public_id(hostel_id)
                .await
                .map_err(|e| e.to_string())?
                .ok_or_else(|| "hostel has no public id".to_string())?;
            let commands: Vec<CommandEnvelope> = pending
                .iter()
                .map(|row| CommandEnvelope {
                    idempotency_key: row.idempotency_key.clone(),
                    command: row.command.clone(),
                })
                .collect();
            let pushed: PushBody = post_json(
                &client,
                &format!("{}/v1/push", config.server_url),
                Some(&token),
                &serde_json::json!({
                    "hostel_public_id": hostel_public_id,
                    "commands": commands,
                }),
            )
            .await?;
            if pushed.results.len() != pending.len() {
                return Err("server ack count did not match the outbox".to_string());
            }
            for (row, ack) in pending.iter().zip(pushed.results.iter()) {
                if ack.status == "acked" {
                    report.pushed += 1;
                    self.db
                        .set_outbox_status(row.id, "acked", None)
                        .await
                        .map_err(|e| e.to_string())?;
                } else {
                    report.conflicts += 1;
                    self.db
                        .set_outbox_status(row.id, "conflict", ack.message.as_deref())
                        .await
                        .map_err(|e| e.to_string())?;
                    rollback_conflict(&self.db, hostel_id, &row.command).await?;
                }
            }
        }

        pull_apply(self, &client, &config, &token, cursor, &mut report).await?;
        if report.pushed > 0 || report.applied > 0 || report.conflicts > 0 {
            report.summary = format!(
                "Synced {} sent, {} received, {} conflicts",
                report.pushed, report.applied, report.conflicts
            );
        }
        Ok(report)
    }

    /// Registration and outbox counts for Settings.
    pub async fn cloud_status(&self) -> Result<CloudStatus, String> {
        let config = read_config(&self.paths);
        let Some(hostel_id) = self
            .db
            .primary_hostel_id()
            .await
            .map_err(|e| e.to_string())?
        else {
            return Ok(CloudStatus {
                server_url: config.as_ref().map(|config| config.server_url.clone()),
                device_prefix: config.as_ref().map(|config| config.device_prefix.clone()),
                cursor: "0".to_string(),
                pending: 0,
                conflicts: Vec::new(),
            });
        };
        let state = self
            .db
            .sync_state(hostel_id)
            .await
            .map_err(|e| e.to_string())?;
        let conflicts = self
            .db
            .sync_conflicts(hostel_id)
            .await
            .map_err(|e| e.to_string())?;
        Ok(CloudStatus {
            server_url: state
                .as_ref()
                .and_then(|row| row.server_url.clone())
                .or_else(|| config.as_ref().map(|config| config.server_url.clone())),
            device_prefix: state
                .as_ref()
                .and_then(|row| row.device_prefix.clone())
                .or_else(|| config.as_ref().map(|config| config.device_prefix.clone())),
            cursor: state
                .as_ref()
                .map(|row| row.cursor.clone())
                .unwrap_or_else(|| "0".to_string()),
            pending: self
                .db
                .pending_sync_count(hostel_id)
                .await
                .map_err(|e| e.to_string())?,
            conflicts: conflicts
                .into_iter()
                .map(|conflict| {
                    if conflict.message.is_empty() {
                        conflict.idempotency_key
                    } else {
                        conflict.message
                    }
                })
                .collect(),
        })
    }

    /// Let this process use the desk after an online sign-in, before a lease file exists.
    fn grant_online_access(&self) {
        let expiry = chrono::Utc::now().date_naive() + chrono::Duration::days(1);
        self.license.set(license::LicensePayload {
            hostel_name: "HostelDesk".to_string(),
            expiry_date: expiry,
            tier: "sync".to_string(),
        });
    }
}

async fn pull_apply(
    desk: &Desk,
    client: &reqwest::Client,
    config: &CloudConfig,
    token: &str,
    cursor: i64,
    report: &mut SyncReport,
) -> Result<i64, String> {
    let local_id = desk
        .db
        .primary_hostel_id()
        .await
        .map_err(|e| e.to_string())?;
    let local_public_id = match local_id {
        Some(id) => desk
            .db
            .hostel_public_id(id)
            .await
            .map_err(|e| e.to_string())?,
        None => None,
    };
    let hostel_public_id = config
        .hostel_public_ids
        .first()
        .cloned()
        .or(local_public_id)
        .ok_or_else(|| "no hostel to pull".to_string())?;
    let pulled: PullBody = post_json(
        client,
        &format!("{}/v1/pull", config.server_url),
        Some(token),
        &serde_json::json!({
            "hostel_public_id": hostel_public_id,
            "cursor": cursor,
        }),
    )
    .await?;
    if !pulled.changes.is_empty() {
        desk.db
            .apply_remote_entities(&pulled.changes)
            .await
            .map_err(|e| e.to_string())?;
        report.applied += pulled.changes.len();
    }
    if let Some(hostel_id) = desk
        .db
        .primary_hostel_id()
        .await
        .map_err(|e| e.to_string())?
    {
        desk.db
            .save_sync_cursor(hostel_id, pulled.cursor)
            .await
            .map_err(|e| e.to_string())?;
        if let Some(lease) = pulled.lease.as_deref() {
            desk.db
                .save_lease(hostel_id, lease)
                .await
                .map_err(|e| e.to_string())?;
            if license::verify_license(lease).is_ok() {
                let _ = desk.activate_license_text(lease);
            }
        }
    }
    Ok(pulled.cursor)
}

async fn rollback_conflict(
    pool: &DbPool,
    hostel_id: i64,
    command: &SyncCommand,
) -> Result<(), String> {
    let SyncCommand::CheckIn {
        allocation_public_id,
        ..
    } = command
    else {
        return Ok(());
    };
    let Some(allocation_id) = pool
        .allocation_id_by_public_id(hostel_id, allocation_public_id)
        .await
        .map_err(|e| e.to_string())?
    else {
        return Ok(());
    };
    match pool.check_out(hostel_id, allocation_id).await {
        Ok(()) => Ok(()),
        Err(error) if error.to_string().contains("not active") => Ok(()),
        Err(error) => Err(error.to_string()),
    }
}

fn http_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .build()
        .map_err(|e| e.to_string())
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
    let response = request.send().await.map_err(|e| e.to_string())?;
    let status = response.status();
    if !status.is_success() {
        let text = response.text().await.unwrap_or_default();
        return Err(format!("server returned {status}: {text}"));
    }
    response.json().await.map_err(|e| e.to_string())
}

fn write_config(paths: &AppPaths, config: &CloudConfig) -> Result<(), String> {
    std::fs::create_dir_all(&paths.data_dir).map_err(|e| e.to_string())?;
    let text = serde_json::to_string_pretty(config).map_err(|e| e.to_string())?;
    std::fs::write(paths.cloud_config_path(), text).map_err(|e| e.to_string())
}

fn read_config(paths: &AppPaths) -> Option<CloudConfig> {
    let text = std::fs::read_to_string(paths.cloud_config_path()).ok()?;
    serde_json::from_str(&text).ok()
}

fn store_token(paths: &AppPaths, token: &str) -> Result<(), String> {
    std::fs::create_dir_all(&paths.data_dir).map_err(|e| e.to_string())?;
    write_token_file(paths, token)?;
    if let Ok(entry) = keyring::Entry::new(KEYRING_SERVICE, KEYRING_USER) {
        let _ = entry.set_password(token);
    }
    Ok(())
}

fn read_token(paths: &AppPaths) -> Option<String> {
    if let Ok(entry) = keyring::Entry::new(KEYRING_SERVICE, KEYRING_USER) {
        if let Ok(token) = entry.get_password() {
            if !token.is_empty() {
                return Some(token);
            }
        }
    }
    std::fs::read_to_string(paths.device_token_path())
        .ok()
        .map(|token| token.trim().to_string())
        .filter(|token| !token.is_empty())
}

fn write_token_file(paths: &AppPaths, token: &str) -> Result<(), String> {
    use std::io::Write;
    let mut options = std::fs::OpenOptions::new();
    options.create(true).write(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(paths.device_token_path())
        .map_err(|e| e.to_string())?;
    file.write_all(token.as_bytes())
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{AuthState, LicenseState};
    use crate::Desk;
    use db::{DbPool, NewCheckIn, NewHostel, NewRoom};

    async fn memory_server() -> String {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind");
        let addr = listener.local_addr().expect("addr");
        tokio::spawn(async move {
            let _ = server::serve_memory(listener).await;
        });
        format!("http://{addr}")
    }

    async fn desk_at(root: &std::path::Path) -> Desk {
        let paths = AppPaths::for_test(root);
        std::fs::create_dir_all(&paths.data_dir).expect("data dir");
        let db = DbPool::open(&paths.database_url()).await.expect("db");
        Desk::from_parts(db, AuthState::new(), LicenseState::default(), paths)
    }

    #[tokio::test]
    async fn second_desk_pulls_a_room_and_a_rejected_check_in_is_rolled_back() {
        let url = memory_server().await;
        let root_a = tempfile::tempdir().expect("temp");
        let desk_a = desk_at(root_a.path()).await;
        desk_a
            .connect_cloud(&url, "a@b.co", "secret-pass", true, "North")
            .await
            .expect("sign up");
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
        desk_a
            .db
            .enqueue_hostel(hostel.id)
            .await
            .expect("queue hostel");
        let room_id = desk_a
            .db
            .add_room(NewRoom {
                hostel_id: hostel.id,
                room_number: "1".into(),
                room_type: "single".into(),
                capacity: 1,
                floor: None,
                semester_rate: "100".into(),
                recess_rate: "40".into(),
            })
            .await
            .expect("room");
        desk_a
            .db
            .enqueue_room(hostel.id, room_id)
            .await
            .expect("queue room");
        let report = desk_a.sync_once().await.expect("sync a");
        assert_eq!(report.conflicts, 0);
        assert!(report.pushed >= 2);

        let root_b = tempfile::tempdir().expect("temp b");
        let desk_b = desk_at(root_b.path()).await;
        desk_b
            .connect_cloud(&url, "a@b.co", "secret-pass", false, "North")
            .await
            .expect("sign in");
        let pulled = desk_b.sync_once().await.expect("sync b");
        assert!(
            pulled.applied >= 2,
            "second desk should receive hostel and room"
        );
        let rooms = desk_b
            .db
            .get_rooms(
                desk_b
                    .db
                    .primary_hostel_id()
                    .await
                    .expect("id")
                    .expect("hostel"),
            )
            .await
            .expect("rooms");
        assert_eq!(rooms.len(), 1);
        assert_eq!(rooms[0].room_number, "1");

        let hostel_a = desk_a
            .db
            .primary_hostel_id()
            .await
            .expect("id")
            .expect("hostel");
        let hostel_b = desk_b
            .db
            .primary_hostel_id()
            .await
            .expect("id")
            .expect("hostel");
        let room_a = desk_a.db.get_rooms(hostel_a).await.expect("rooms")[0].id;
        let room_b = rooms[0].id;
        let stay_a = desk_a
            .db
            .check_in(NewCheckIn {
                hostel_id: hostel_a,
                room_id: room_a,
                resident_id: None,
                full_name: Some("Ada".into()),
                phone: None,
                email: None,
                id_number: None,
                next_of_kin: None,
                next_of_kin_phone: None,
                photo_path: None,
                rate_agreed: "100".into(),
                term: "semester_1".into(),
                academic_year: "2026/2027".into(),
            })
            .await
            .expect("check in a");
        desk_a
            .db
            .enqueue_check_in(hostel_a, stay_a)
            .await
            .expect("queue a");
        let stay_b = desk_b
            .db
            .check_in(NewCheckIn {
                hostel_id: hostel_b,
                room_id: room_b,
                resident_id: None,
                full_name: Some("Grace".into()),
                phone: None,
                email: None,
                id_number: None,
                next_of_kin: None,
                next_of_kin_phone: None,
                photo_path: None,
                rate_agreed: "100".into(),
                term: "semester_1".into(),
                academic_year: "2026/2027".into(),
            })
            .await
            .expect("check in b");
        desk_b
            .db
            .enqueue_check_in(hostel_b, stay_b)
            .await
            .expect("queue b");

        let synced_a = desk_a.sync_once().await.expect("push a stay");
        assert_eq!(synced_a.conflicts, 0);
        let synced_b = desk_b.sync_once().await.expect("push b stay");
        assert_eq!(synced_b.conflicts, 1);
        let status: String = sqlx::query_scalar("SELECT status FROM allocations WHERE id = ?1")
            .bind(stay_b)
            .fetch_one(&desk_b.db.inner)
            .await
            .expect("status");
        assert_eq!(status, "checked_out");
    }
}
