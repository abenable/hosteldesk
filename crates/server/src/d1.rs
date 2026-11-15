//! Cloudflare Worker entry and the D1-backed [`SyncStore`].
//!
//! Every list a query filters on is bound as one JSON array and expanded with
//! `json_each`, because D1 caps a statement at 100 bound parameters.

use crate::{
    apply_batch, dev_signing_key, dump_from, entity_key, hash_password, normalize_email,
    random_device_prefix, random_id, route, sign_lease, signing_key_from_base64, token_hash,
    verify_password, ApiError, SyncStore, DUMMY_HASH,
};
use ed25519_dalek::SigningKey;
use protocol::{
    AuthResponse, BatchRefs, EnrollRequest, EnrollResponse, Entity, Ledger, PullRequest,
    PullResponse, PushRequest, PushResponse, SignInRequest, SignUpRequest, PULL_PAGE_SIZE,
};
use serde::Deserialize;
use std::collections::HashSet;
use worker::d1::{D1Database, D1PreparedStatement, D1Result};
use worker::wasm_bindgen::JsValue;
use worker::{event, Context, Env, Request, Response};

/// Worker entry: route the request over D1.
#[event(fetch)]
async fn fetch(mut request: Request, env: Env, _ctx: Context) -> worker::Result<Response> {
    let signing_key = env
        .secret("HOSTELDESK_SIGNING_KEY")
        .ok()
        .and_then(|secret| signing_key_from_base64(&secret.to_string()))
        .or_else(dev_signing_key);
    let store = D1Store {
        db: env.d1("DB")?,
        signing_key,
    };
    let method = request.method().to_string();
    let path = request.path();
    let authorization = request.headers().get("authorization")?;
    let body = request.bytes().await?;
    let reply = route(&store, &method, &path, authorization.as_deref(), &body).await;
    let content_type = if reply.status == 200 && path != "/health" {
        "application/json"
    } else {
        "text/plain; charset=utf-8"
    };
    let mut response = Response::ok(reply.body)?.with_status(reply.status);
    response.headers_mut().set("content-type", content_type)?;
    Ok(response)
}

struct D1Store {
    db: D1Database,
    signing_key: Option<SigningKey>,
}

fn text(value: &str) -> JsValue {
    JsValue::from_str(value)
}

// ponytail: D1 binds numbers as JS doubles. Only cursors and counters go
// through here, never money (money is always a decimal string).
fn int(value: i64) -> JsValue {
    JsValue::from_f64(value as f64)
}

fn json_list<'a>(values: impl IntoIterator<Item = &'a String>) -> Result<JsValue, ApiError> {
    serde_json::to_string(&values.into_iter().collect::<Vec<_>>())
        .map(|json| text(&json))
        .map_err(storage)
}

fn storage(error: impl std::fmt::Display) -> ApiError {
    ApiError::Storage(error.to_string())
}

fn is_unique_violation(error: &ApiError) -> bool {
    matches!(error, ApiError::Storage(detail) if detail.contains("UNIQUE constraint failed"))
}

#[derive(Deserialize)]
struct OrgRow {
    org_id: String,
}

#[derive(Deserialize)]
struct BodyRow {
    body: String,
}

impl D1Store {
    fn statement(&self, sql: &str, binds: &[JsValue]) -> Result<D1PreparedStatement, ApiError> {
        self.db.prepare(sql).bind(binds).map_err(storage)
    }

    async fn batch(&self, statements: Vec<D1PreparedStatement>) -> Result<Vec<D1Result>, ApiError> {
        self.db.batch(statements).await.map_err(storage)
    }

    async fn first<T: for<'de> Deserialize<'de>>(
        &self,
        sql: &str,
        binds: &[JsValue],
    ) -> Result<Option<T>, ApiError> {
        self.statement(sql, binds)?
            .first::<T>(None)
            .await
            .map_err(storage)
    }

    async fn new_session(&self, org_id: &str) -> Result<String, ApiError> {
        let access_token = random_id();
        self.statement(
            "INSERT INTO sessions (token_hash, org_id, expires_at) \
             VALUES (?1, ?2, datetime('now', '+30 days'))",
            &[text(&token_hash(&access_token)), text(org_id)],
        )?
        .run()
        .await
        .map_err(storage)?;
        Ok(access_token)
    }

    async fn device_org(&self, refresh_token: &str) -> Result<String, ApiError> {
        self.first::<OrgRow>(
            "SELECT org_id FROM devices WHERE token_hash = ?1 AND revoked_at IS NULL",
            &[text(&token_hash(refresh_token))],
        )
        .await?
        .map(|row| row.org_id)
        .ok_or(ApiError::Unauthorized)
    }

    /// Read only the rows `refs` needs. Returns the hostel's owner (None for a
    /// new hostel), the scoped ledger, and the next free cursor.
    async fn load_scoped(
        &self,
        hostel: &str,
        refs: &BatchRefs,
    ) -> Result<(Option<String>, Ledger, i64), ApiError> {
        #[derive(Deserialize)]
        struct KeyRow {
            idempotency_key: String,
        }
        #[derive(Deserialize)]
        struct NextRow {
            next: i64,
        }
        let h = text(hostel);
        let first = self
            .batch(vec![
                self.statement("SELECT org_id FROM hostels WHERE public_id = ?1", std::slice::from_ref(&h))?,
                self.statement(
                    "SELECT body FROM entities WHERE hostel_public_id = ?1 \
                     AND kind IN ('hostel', 'preferences')",
                    std::slice::from_ref(&h),
                )?,
                self.statement(
                    "SELECT body FROM entities WHERE hostel_public_id = ?1 AND kind = 'allocation' \
                     AND public_id IN (SELECT value FROM json_each(?2))",
                    &[h.clone(), json_list(&refs.allocations)?],
                )?,
                self.statement(
                    "SELECT body FROM entities WHERE hostel_public_id = ?1 AND kind = 'resident' \
                     AND public_id IN (SELECT value FROM json_each(?2))",
                    &[h.clone(), json_list(&refs.residents)?],
                )?,
                self.statement(
                    "SELECT body FROM entities WHERE hostel_public_id = ?1 AND kind = 'payment' \
                     AND public_id IN (SELECT value FROM json_each(?2))",
                    &[h.clone(), json_list(&refs.payments)?],
                )?,
                self.statement(
                    "SELECT body FROM entities WHERE hostel_public_id = ?1 AND kind = 'receipt' \
                     AND lookup_key IN (SELECT value FROM json_each(?2))",
                    &[h.clone(), json_list(&refs.receipt_numbers)?],
                )?,
                self.statement(
                    "SELECT idempotency_key FROM applied_commands WHERE hostel_public_id = ?1 \
                     AND idempotency_key IN (SELECT value FROM json_each(?2))",
                    &[h.clone(), json_list(&refs.idempotency_keys)?],
                )?,
                self.statement(
                    "SELECT COALESCE(MAX(cursor), 0) + 1 AS next FROM changes \
                     WHERE hostel_public_id = ?1",
                    std::slice::from_ref(&h),
                )?,
            ])
            .await?;
        let owner = first[0]
            .results::<OrgRow>()
            .map_err(storage)?
            .pop()
            .map(|row| row.org_id);
        let mut entities = Vec::new();
        for result in &first[1..6] {
            entities.extend(bodies(result)?);
        }
        let applied = first[6]
            .results::<KeyRow>()
            .map_err(storage)?
            .into_iter()
            .map(|row| row.idempotency_key)
            .collect();
        let next_cursor = first[7]
            .results::<NextRow>()
            .map_err(storage)?
            .pop()
            .map_or(1, |row| row.next);

        // Rooms the batch names, rooms of the allocations it touches, and rooms
        // holding a number it wants; plus active stays in the first two sets.
        let mut rooms: HashSet<String> = refs.rooms.clone();
        for entity in &entities {
            if let Entity::Allocation(row) = entity {
                rooms.insert(row.room_public_id.clone());
            }
        }
        let second = self
            .batch(vec![
                self.statement(
                    "SELECT body FROM entities WHERE hostel_public_id = ?1 AND kind = 'room' \
                     AND (public_id IN (SELECT value FROM json_each(?2)) \
                          OR lookup_key IN (SELECT value FROM json_each(?3)))",
                    &[h.clone(), json_list(&rooms)?, json_list(&refs.room_numbers)?],
                )?,
                self.statement(
                    "SELECT body FROM entities WHERE hostel_public_id = ?1 AND kind = 'allocation' \
                     AND active = 1 AND lookup_key IN (SELECT value FROM json_each(?2))",
                    &[h, json_list(&rooms)?],
                )?,
            ])
            .await?;
        // An allocation can come back twice (named and active in a named
        // room); `Ledger::from_dump` keys rows by id, so that is harmless.
        entities.extend(bodies(&second[0])?);
        entities.extend(bodies(&second[1])?);
        Ok((
            owner,
            Ledger::from_dump(dump_from(entities, applied)),
            next_cursor,
        ))
    }

    async fn write(
        &self,
        hostel: &str,
        org_id: &str,
        new_hostel: bool,
        outcome: &crate::BatchOutcome,
    ) -> Result<(), ApiError> {
        let mut entities = Vec::new();
        let mut changes = Vec::new();
        for (cursor, entity) in &outcome.changes {
            let body = serde_json::to_string(entity).map_err(storage)?;
            let (kind, public_id, lookup_key, active) = entity_key(entity);
            entities.push(serde_json::json!({
                "kind": kind,
                "public_id": public_id,
                "lookup_key": lookup_key,
                "active": i64::from(active),
                "body": body,
            }));
            changes.push(serde_json::json!({ "cursor": cursor, "body": body }));
        }
        let h = text(hostel);
        let mut statements = Vec::new();
        if new_hostel {
            // A plain INSERT: if another push created this hostel meanwhile,
            // the batch fails and the retry re-checks ownership.
            statements.push(self.statement(
                "INSERT INTO hostels (public_id, org_id) VALUES (?1, ?2)",
                &[h.clone(), text(org_id)],
            )?);
        }
        statements.push(self.statement(
            "INSERT INTO entities (hostel_public_id, kind, public_id, lookup_key, active, body) \
             SELECT ?1, json_extract(value, '$.kind'), json_extract(value, '$.public_id'), json_extract(value, '$.lookup_key'), \
                    json_extract(value, '$.active'), json_extract(value, '$.body') \
             FROM json_each(?2) WHERE true \
             ON CONFLICT (hostel_public_id, kind, public_id) DO UPDATE SET \
                 lookup_key = excluded.lookup_key, active = excluded.active, body = excluded.body",
            &[h.clone(), text(&serde_json::to_string(&entities).map_err(storage)?)],
        )?);
        statements.push(self.statement(
            "INSERT INTO applied_commands (hostel_public_id, idempotency_key) \
             SELECT ?1, value FROM json_each(?2)",
            &[h.clone(), json_list(&outcome.applied)?],
        )?);
        statements.push(self.statement(
            "INSERT INTO changes (hostel_public_id, cursor, body) \
             SELECT ?1, json_extract(value, '$.cursor'), json_extract(value, '$.body') FROM json_each(?2)",
            &[h, text(&serde_json::to_string(&changes).map_err(storage)?)],
        )?);
        self.batch(statements).await.map(|_| ())
    }
}

fn bodies(result: &D1Result) -> Result<Vec<Entity>, ApiError> {
    result
        .results::<BodyRow>()
        .map_err(storage)?
        .into_iter()
        .map(|row| serde_json::from_str::<Entity>(&row.body).map_err(storage))
        .collect()
}

impl SyncStore for D1Store {
    async fn sign_up(&self, request: SignUpRequest) -> Result<AuthResponse, ApiError> {
        let email = normalize_email(&request.email);
        let taken: Option<OrgRow> = self
            .first(
                "SELECT org_id FROM accounts WHERE email = ?1",
                &[text(&email)],
            )
            .await?;
        if taken.is_some() {
            return Err(ApiError::BadRequest("email is already registered".into()));
        }
        let password_hash = hash_password(&request.password)?;
        let org_id = random_id();
        let access_token = random_id();
        let result = self
            .batch(vec![
                self.statement(
                    "INSERT INTO organizations (id, name) VALUES (?1, ?2)",
                    &[text(&org_id), text(request.organization_name.trim())],
                )?,
                self.statement(
                    "INSERT INTO accounts (email, org_id, password_hash) VALUES (?1, ?2, ?3)",
                    &[text(&email), text(&org_id), text(&password_hash)],
                )?,
                self.statement(
                    "INSERT INTO sessions (token_hash, org_id, expires_at) \
                     VALUES (?1, ?2, datetime('now', '+30 days'))",
                    &[text(&token_hash(&access_token)), text(&org_id)],
                )?,
            ])
            .await;
        match result {
            Err(error) if is_unique_violation(&error) => {
                Err(ApiError::BadRequest("email is already registered".into()))
            }
            Err(error) => Err(error),
            Ok(_) => Ok(AuthResponse {
                access_token,
                organization_id: org_id,
            }),
        }
    }

    async fn sign_in(&self, request: SignInRequest) -> Result<AuthResponse, ApiError> {
        #[derive(Deserialize)]
        struct AccountRow {
            org_id: String,
            password_hash: String,
            locked: i64,
        }
        let email = normalize_email(&request.email);
        let account: Option<AccountRow> = self
            .first(
                "SELECT org_id, password_hash, \
                 (locked_until IS NOT NULL AND locked_until > datetime('now')) AS locked \
                 FROM accounts WHERE email = ?1",
                &[text(&email)],
            )
            .await?;
        let Some(account) = account else {
            verify_password(&request.password, DUMMY_HASH);
            return Err(ApiError::Unauthorized);
        };
        if account.locked != 0 {
            return Err(ApiError::TooManyRequests);
        }
        if !verify_password(&request.password, &account.password_hash) {
            // The fifth failure in a row locks the email for 15 minutes and
            // resets the count. Right-hand sides read the pre-update row.
            self.statement(
                "UPDATE accounts SET \
                 failed_logins = CASE WHEN failed_logins + 1 >= 5 THEN 0 ELSE failed_logins + 1 END, \
                 locked_until = CASE WHEN failed_logins + 1 >= 5 \
                     THEN datetime('now', '+15 minutes') ELSE locked_until END \
                 WHERE email = ?1",
                &[text(&email)],
            )?
            .run()
            .await
            .map_err(storage)?;
            return Err(ApiError::Unauthorized);
        }
        self.statement(
            "UPDATE accounts SET failed_logins = 0, locked_until = NULL WHERE email = ?1",
            &[text(&email)],
        )?
        .run()
        .await
        .map_err(storage)?;
        let access_token = self.new_session(&account.org_id).await?;
        Ok(AuthResponse {
            access_token,
            organization_id: account.org_id,
        })
    }

    async fn enroll(
        &self,
        access_token: &str,
        request: EnrollRequest,
    ) -> Result<EnrollResponse, ApiError> {
        let org_id = self
            .first::<OrgRow>(
                "SELECT org_id FROM sessions WHERE token_hash = ?1 AND expires_at > datetime('now')",
                &[text(&token_hash(access_token))],
            )
            .await?
            .map(|row| row.org_id)
            .ok_or(ApiError::Unauthorized)?;
        let name: String = request.device_name.trim().chars().take(100).collect();
        let mut attempt = 0;
        let (device_id, device_prefix, refresh_token) = loop {
            let device_id = random_id();
            let device_prefix = random_device_prefix();
            let refresh_token = random_id();
            let inserted = self
                .statement(
                    "INSERT INTO devices (token_hash, device_id, org_id, device_prefix, name) \
                     VALUES (?1, ?2, ?3, ?4, ?5)",
                    &[
                        text(&token_hash(&refresh_token)),
                        text(&device_id),
                        text(&org_id),
                        text(&device_prefix),
                        text(&name),
                    ],
                )?
                .run()
                .await
                .map_err(storage);
            match inserted {
                Ok(_) => break (device_id, device_prefix, refresh_token),
                Err(error) if is_unique_violation(&error) && attempt < 3 => attempt += 1,
                Err(error) => return Err(error),
            }
        };
        #[derive(Deserialize)]
        struct HostelRow {
            public_id: String,
        }
        let hostel_public_ids = self
            .statement(
                "SELECT public_id FROM hostels WHERE org_id = ?1 ORDER BY public_id",
                &[text(&org_id)],
            )?
            .all()
            .await
            .map_err(storage)?
            .results::<HostelRow>()
            .map_err(storage)?
            .into_iter()
            .map(|row| row.public_id)
            .collect();
        Ok(EnrollResponse {
            device_id,
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
        let org_id = self.device_org(refresh_token).await?;
        let refs = BatchRefs::of(&request.commands);
        let hostel = request.hostel_public_id.as_str();
        // ponytail: optimistic concurrency. The changes primary key rejects a
        // second push built from the same snapshot (same next cursor); the
        // whole D1 batch rolls back and we re-read and re-apply. Three tries
        // is plenty for a handful of desks per hostel.
        let mut attempt = 0;
        loop {
            let (owner, mut ledger, next_cursor) = self.load_scoped(hostel, &refs).await?;
            if owner.as_deref().is_some_and(|owner| owner != org_id) {
                return Err(ApiError::Unauthorized);
            }
            let outcome = apply_batch(&mut ledger, hostel, &request.commands, next_cursor);
            if outcome.changes.is_empty() {
                return Ok(PushResponse {
                    results: outcome.results,
                });
            }
            match self.write(hostel, &org_id, owner.is_none(), &outcome).await {
                Ok(()) => {
                    return Ok(PushResponse {
                        results: outcome.results,
                    })
                }
                Err(error) if is_unique_violation(&error) && attempt < 3 => attempt += 1,
                Err(error) => return Err(error),
            }
        }
    }

    async fn pull(
        &self,
        refresh_token: &str,
        request: PullRequest,
    ) -> Result<PullResponse, ApiError> {
        #[derive(Deserialize)]
        struct ChangeRow {
            cursor: i64,
            body: String,
        }
        #[derive(Deserialize)]
        struct LicenseRow {
            licensed: i64,
            lease_expiry: String,
        }
        let org_id = self.device_org(refresh_token).await?;
        let h = text(&request.hostel_public_id);
        let results = self
            .batch(vec![
                self.statement(
                    "SELECT org_id FROM hostels WHERE public_id = ?1",
                    std::slice::from_ref(&h),
                )?,
                self.statement(
                    "SELECT cursor, body FROM changes WHERE hostel_public_id = ?1 AND cursor > ?2 \
                     ORDER BY cursor LIMIT ?3",
                    &[h.clone(), int(request.cursor), int(PULL_PAGE_SIZE + 1)],
                )?,
                self.statement(
                    "SELECT licensed, date('now', '+14 days') AS lease_expiry \
                     FROM organizations WHERE id = ?1",
                    &[text(&org_id)],
                )?,
                self.statement(
                    "SELECT body FROM entities WHERE hostel_public_id = ?1 AND kind = 'hostel'",
                    &[h],
                )?,
            ])
            .await?;
        let owner = results[0].results::<OrgRow>().map_err(storage)?.pop();
        match owner {
            None => {
                return Ok(PullResponse {
                    cursor: request.cursor,
                    changes: Vec::new(),
                    has_more: false,
                    lease: None,
                })
            }
            Some(owner) if owner.org_id != org_id => return Err(ApiError::Unauthorized),
            Some(_) => {}
        }
        let mut rows = results[1].results::<ChangeRow>().map_err(storage)?;
        let has_more = rows.len() > PULL_PAGE_SIZE as usize;
        rows.truncate(PULL_PAGE_SIZE as usize);
        let cursor = rows.last().map_or(request.cursor, |row| row.cursor);
        let changes = rows
            .iter()
            .map(|row| serde_json::from_str::<Entity>(&row.body).map_err(storage))
            .collect::<Result<Vec<_>, _>>()?;
        let license = results[2].results::<LicenseRow>().map_err(storage)?.pop();
        let lease = match (&self.signing_key, license) {
            (Some(key), Some(row)) if row.licensed == 1 => {
                let name = bodies(&results[3])?
                    .into_iter()
                    .find_map(|entity| match entity {
                        Entity::Hostel(hostel) => Some(hostel.name),
                        _ => None,
                    })
                    .unwrap_or_else(|| "HostelDesk".to_string());
                chrono::NaiveDate::parse_from_str(&row.lease_expiry, "%Y-%m-%d")
                    .ok()
                    .and_then(|expiry| sign_lease(&name, expiry, key))
            }
            _ => None,
        };
        Ok(PullResponse {
            cursor,
            changes,
            has_more,
            lease,
        })
    }
}
