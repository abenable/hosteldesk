//! Cloudflare Worker entry and the Postgres-backed [`SyncStore`].
//!
//! The database is Neon Postgres, reached through a Cloudflare Hyperdrive
//! binding (`HYPERDRIVE`), which pools connections close to Neon. Each request
//! opens one `tokio-postgres` client over the Hyperdrive socket.
//!
//! A push runs in one transaction that first locks the hostel row
//! (`SELECT … FOR UPDATE`), so two desks pushing to the same hostel apply one
//! after the other against current data.

use crate::{
    apply_batch, dev_signing_key, dump_from, entity_key, hash_password, normalize_email,
    random_device_prefix, random_id, route, sign_lease, signing_key_from_base64, token_hash,
    verify_password, ApiError, BatchOutcome, SyncStore, DUMMY_HASH,
};
use ed25519_dalek::SigningKey;
use protocol::{
    AuthResponse, BatchRefs, EnrollRequest, EnrollResponse, Entity, Ledger, PullRequest,
    PullResponse, PushRequest, PushResponse, SignInRequest, SignUpRequest, PULL_PAGE_SIZE,
};
use std::collections::{HashMap, HashSet};
use std::str::FromStr;
use tokio_postgres::error::SqlState;
use tokio_postgres::types::ToSql;
use tokio_postgres::Client;
use worker::postgres_tls::PassthroughTls;
use worker::{event, Context, Env, Request, Response};

/// Worker entry: open a Postgres client through Hyperdrive and route the request.
#[event(fetch)]
async fn fetch(mut request: Request, env: Env, _ctx: Context) -> worker::Result<Response> {
    let method = request.method().to_string();
    let path = request.path();
    if path == "/health" {
        // Answered without touching the database, so a suspended Neon
        // compute is not woken by uptime checks.
        return text_response(200, "ok".into());
    }
    let signing_key = env
        .secret("HOSTELDESK_SIGNING_KEY")
        .ok()
        .and_then(|secret| signing_key_from_base64(&secret.to_string()))
        .or_else(dev_signing_key);
    let client = match connect(&env).await {
        Ok(client) => client,
        Err(error) => {
            worker::console_error!("database connection failed: {error}");
            return text_response(503, "database unavailable, try again".into());
        }
    };
    let store = PgStore {
        client,
        signing_key,
    };
    let authorization = request.headers().get("authorization")?;
    let body = request.bytes().await?;
    let reply = route(&store, &method, &path, authorization.as_deref(), &body).await;
    let mut response = text_response(reply.status, reply.body)?;
    if reply.status == 200 {
        response
            .headers_mut()
            .set("content-type", "application/json")?;
    }
    Ok(response)
}

fn text_response(status: u16, body: String) -> worker::Result<Response> {
    let mut response = Response::ok(body)?.with_status(status);
    response
        .headers_mut()
        .set("content-type", "text/plain; charset=utf-8")?;
    Ok(response)
}

async fn connect(env: &Env) -> Result<Client, String> {
    let hyperdrive = env.hyperdrive("HYPERDRIVE").map_err(|e| e.to_string())?;
    let config = tokio_postgres::Config::from_str(&hyperdrive.connection_string())
        .map_err(|e| e.to_string())?;
    let socket = hyperdrive.connect().map_err(|e| e.to_string())?;
    let (client, connection) = config
        .connect_raw(socket, PassthroughTls)
        .await
        .map_err(|e| e.to_string())?;
    worker::wasm_bindgen_futures::spawn_local(async move {
        if let Err(error) = connection.await {
            worker::console_error!("postgres connection error: {error}");
        }
    });
    Ok(client)
}

struct PgStore {
    client: Client,
    signing_key: Option<SigningKey>,
}

fn db(error: tokio_postgres::Error) -> ApiError {
    ApiError::Storage(error.to_string())
}

fn is_unique_violation(error: &tokio_postgres::Error) -> bool {
    error.code() == Some(&SqlState::UNIQUE_VIOLATION)
}

fn list(set: &HashSet<String>) -> Vec<&str> {
    set.iter().map(String::as_str).collect()
}

/// Rows of `SELECT body::text`, parsed back into entities.
async fn bodies(
    client: &Client,
    sql: &str,
    params: &[&(dyn ToSql + Sync)],
) -> Result<Vec<Entity>, ApiError> {
    client
        .query(sql, params)
        .await
        .map_err(db)?
        .iter()
        .map(|row| {
            serde_json::from_str::<Entity>(row.get::<_, &str>(0))
                .map_err(|e| ApiError::Storage(e.to_string()))
        })
        .collect()
}

impl PgStore {
    async fn device_org(&self, refresh_token: &str) -> Result<String, ApiError> {
        self.client
            .query_opt(
                "SELECT org_id FROM devices WHERE token_hash = $1 AND revoked_at IS NULL",
                &[&token_hash(refresh_token)],
            )
            .await
            .map_err(db)?
            .map(|row| row.get(0))
            .ok_or(ApiError::Unauthorized)
    }

    async fn new_session(&self, org_id: &str) -> Result<String, ApiError> {
        let access_token = random_id();
        self.client
            .execute(
                "INSERT INTO sessions (token_hash, org_id, expires_at) \
                 VALUES ($1, $2, now() + interval '30 days')",
                &[&token_hash(&access_token), &org_id],
            )
            .await
            .map_err(db)?;
        Ok(access_token)
    }

    /// Read only the rows `refs` needs. Runs inside the push's `BEGIN`.
    async fn load_scoped(&self, hostel: &str, refs: &BatchRefs) -> Result<(Ledger, i64), ApiError> {
        let mut entities = bodies(
            &self.client,
            "SELECT body::text FROM entities WHERE hostel_public_id = $1 \
             AND kind IN ('hostel', 'preferences')",
            &[&hostel],
        )
        .await?;
        for (kind, ids) in [
            ("allocation", &refs.allocations),
            ("resident", &refs.residents),
            ("payment", &refs.payments),
        ] {
            entities.extend(
                bodies(
                    &self.client,
                    "SELECT body::text FROM entities WHERE hostel_public_id = $1 \
                     AND kind = $2 AND public_id = ANY($3)",
                    &[&hostel, &kind, &list(ids)],
                )
                .await?,
            );
        }
        entities.extend(
            bodies(
                &self.client,
                "SELECT body::text FROM entities WHERE hostel_public_id = $1 \
                 AND kind = 'receipt' AND lookup_key = ANY($2)",
                &[&hostel, &list(&refs.receipt_numbers)],
            )
            .await?,
        );
        // Rooms the batch names, rooms of the stays it touches, and rooms
        // holding a number it wants; plus active stays in the first two.
        let mut rooms = refs.rooms.clone();
        for entity in &entities {
            if let Entity::Allocation(row) = entity {
                rooms.insert(row.room_public_id.clone());
            }
        }
        entities.extend(
            bodies(
                &self.client,
                "SELECT body::text FROM entities WHERE hostel_public_id = $1 AND kind = 'room' \
                 AND (public_id = ANY($2) OR lookup_key = ANY($3))",
                &[&hostel, &list(&rooms), &list(&refs.room_numbers)],
            )
            .await?,
        );
        // A stay can come back twice (named, and active in a named room);
        // `Ledger::from_dump` keys rows by id, so that is harmless.
        entities.extend(
            bodies(
                &self.client,
                "SELECT body::text FROM entities WHERE hostel_public_id = $1 \
                 AND kind = 'allocation' AND active AND lookup_key = ANY($2)",
                &[&hostel, &list(&rooms)],
            )
            .await?,
        );
        let applied = self
            .client
            .query(
                "SELECT idempotency_key FROM applied_commands \
                 WHERE hostel_public_id = $1 AND idempotency_key = ANY($2)",
                &[&hostel, &list(&refs.idempotency_keys)],
            )
            .await
            .map_err(db)?
            .iter()
            .map(|row| row.get(0))
            .collect();
        let next_cursor: i64 = self
            .client
            .query_one(
                "SELECT COALESCE(MAX(cursor), 0) + 1 FROM changes WHERE hostel_public_id = $1",
                &[&hostel],
            )
            .await
            .map_err(db)?
            .get(0);
        Ok((Ledger::from_dump(dump_from(entities, applied)), next_cursor))
    }

    /// Persist what [`apply_batch`] produced. Runs inside the push's `BEGIN`.
    async fn write(&self, hostel: &str, outcome: &BatchOutcome) -> Result<(), ApiError> {
        // One row per entity: an upsert may not touch the same row twice.
        let mut latest: HashMap<(&str, &str), (Option<&str>, bool, String)> = HashMap::new();
        let mut cursors = Vec::with_capacity(outcome.changes.len());
        let mut change_bodies = Vec::with_capacity(outcome.changes.len());
        for (cursor, entity) in &outcome.changes {
            let body =
                serde_json::to_string(entity).map_err(|e| ApiError::Storage(e.to_string()))?;
            let (kind, public_id, lookup_key, active) = entity_key(entity);
            latest.insert((kind, public_id), (lookup_key, active, body.clone()));
            cursors.push(*cursor);
            change_bodies.push(body);
        }
        let mut kinds = Vec::new();
        let mut ids = Vec::new();
        let mut keys = Vec::new();
        let mut actives = Vec::new();
        let mut entity_bodies = Vec::new();
        for ((kind, id), (key, active, body)) in latest {
            kinds.push(kind);
            ids.push(id);
            keys.push(key);
            actives.push(active);
            entity_bodies.push(body);
        }
        self.client.execute(
            "INSERT INTO entities (hostel_public_id, kind, public_id, lookup_key, active, body) \
             SELECT $1, k, p, l, a, b::jsonb \
             FROM unnest($2::text[], $3::text[], $4::text[], $5::bool[], $6::text[]) AS t(k, p, l, a, b) \
             ON CONFLICT (hostel_public_id, kind, public_id) DO UPDATE SET \
                 lookup_key = excluded.lookup_key, active = excluded.active, body = excluded.body",
            &[&hostel, &kinds, &ids, &keys, &actives, &entity_bodies],
        )
        .await
        .map_err(db)?;
        self.client
            .execute(
                "INSERT INTO applied_commands (hostel_public_id, idempotency_key) \
             SELECT $1, unnest($2::text[])",
                &[&hostel, &outcome.applied],
            )
            .await
            .map_err(db)?;
        self.client
            .execute(
                "INSERT INTO changes (hostel_public_id, cursor, body) \
             SELECT $1, c, b::jsonb FROM unnest($2::int8[], $3::text[]) AS t(c, b)",
                &[&hostel, &cursors, &change_bodies],
            )
            .await
            .map_err(db)?;
        Ok(())
    }
}

impl SyncStore for PgStore {
    async fn sign_up(&self, request: SignUpRequest) -> Result<AuthResponse, ApiError> {
        let email = normalize_email(&request.email);
        let taken = self
            .client
            .query_opt("SELECT 1 FROM accounts WHERE email = $1", &[&email])
            .await
            .map_err(db)?;
        if taken.is_some() {
            return Err(ApiError::BadRequest("email is already registered".into()));
        }
        let password_hash = hash_password(&request.password)?;
        let org_id = random_id();
        let access_token = random_id();
        // One statement, so the organization, account and first session are
        // created together or not at all.
        let result = self
            .client
            .execute(
                "WITH org AS (INSERT INTO organizations (id, name) VALUES ($1, $2)), \
                      acc AS (INSERT INTO accounts (email, org_id, password_hash) \
                              VALUES ($3, $1, $4)) \
                 INSERT INTO sessions (token_hash, org_id, expires_at) \
                 VALUES ($5, $1, now() + interval '30 days')",
                &[
                    &org_id,
                    &request.organization_name.trim(),
                    &email,
                    &password_hash,
                    &token_hash(&access_token),
                ],
            )
            .await
            .map(|_| ());
        match result {
            Err(error) if is_unique_violation(&error) => {
                Err(ApiError::BadRequest("email is already registered".into()))
            }
            Err(error) => Err(db(error)),
            Ok(()) => Ok(AuthResponse {
                access_token,
                organization_id: org_id,
            }),
        }
    }

    async fn sign_in(&self, request: SignInRequest) -> Result<AuthResponse, ApiError> {
        let email = normalize_email(&request.email);
        let account = self
            .client
            .query_opt(
                "SELECT org_id, password_hash, coalesce(locked_until > now(), false) \
                 FROM accounts WHERE email = $1",
                &[&email],
            )
            .await
            .map_err(db)?;
        let Some(account) = account else {
            verify_password(&request.password, DUMMY_HASH);
            return Err(ApiError::Unauthorized);
        };
        let (org_id, password_hash, locked): (String, String, bool) =
            (account.get(0), account.get(1), account.get(2));
        if locked {
            return Err(ApiError::TooManyRequests);
        }
        if !verify_password(&request.password, &password_hash) {
            // The fifth failure in a row locks the email for 15 minutes and
            // resets the count. Right-hand sides read the pre-update row.
            self.client
                .execute(
                    "UPDATE accounts SET \
                     failed_logins = CASE WHEN failed_logins + 1 >= 5 THEN 0 ELSE failed_logins + 1 END, \
                     locked_until = CASE WHEN failed_logins + 1 >= 5 \
                         THEN now() + interval '15 minutes' ELSE locked_until END \
                     WHERE email = $1",
                    &[&email],
                )
                .await
                .map_err(db)?;
            return Err(ApiError::Unauthorized);
        }
        self.client
            .execute(
                "UPDATE accounts SET failed_logins = 0, locked_until = NULL WHERE email = $1",
                &[&email],
            )
            .await
            .map_err(db)?;
        let access_token = self.new_session(&org_id).await?;
        Ok(AuthResponse {
            access_token,
            organization_id: org_id,
        })
    }

    async fn enroll(
        &self,
        access_token: &str,
        request: EnrollRequest,
    ) -> Result<EnrollResponse, ApiError> {
        let org_id: String = self
            .client
            .query_opt(
                "SELECT org_id FROM sessions WHERE token_hash = $1 AND expires_at > now()",
                &[&token_hash(access_token)],
            )
            .await
            .map_err(db)?
            .map(|row| row.get(0))
            .ok_or(ApiError::Unauthorized)?;
        let name: String = request.device_name.trim().chars().take(100).collect();
        let mut attempt = 0;
        let (device_id, device_prefix, refresh_token) = loop {
            let (device_id, device_prefix, refresh_token) =
                (random_id(), random_device_prefix(), random_id());
            let inserted = self
                .client
                .execute(
                    "INSERT INTO devices (token_hash, device_id, org_id, device_prefix, name) \
                     VALUES ($1, $2, $3, $4, $5)",
                    &[
                        &token_hash(&refresh_token),
                        &device_id,
                        &org_id,
                        &device_prefix,
                        &name,
                    ],
                )
                .await;
            match inserted {
                Ok(_) => break (device_id, device_prefix, refresh_token),
                Err(error) if is_unique_violation(&error) && attempt < 3 => attempt += 1,
                Err(error) => return Err(db(error)),
            }
        };
        let hostel_public_ids = self
            .client
            .query(
                "SELECT public_id FROM hostels WHERE org_id = $1 ORDER BY public_id",
                &[&org_id],
            )
            .await
            .map_err(db)?
            .iter()
            .map(|row| row.get(0))
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
        // `Client::transaction` needs `&mut`. This client is this request's
        // own connection, so a plain BEGIN … COMMIT on it is the same thing.
        self.client.batch_execute("BEGIN").await.map_err(db)?;
        let result = async {
            self.client
                .execute(
                    "INSERT INTO hostels (public_id, org_id) VALUES ($1, $2) \
                 ON CONFLICT (public_id) DO NOTHING",
                    &[&hostel, &org_id],
                )
                .await
                .map_err(db)?;
            let owner: String = self
                .client
                .query_one(
                    "SELECT org_id FROM hostels WHERE public_id = $1 FOR UPDATE",
                    &[&hostel],
                )
                .await
                .map_err(db)?
                .get(0);
            if owner != org_id {
                return Err(ApiError::Unauthorized);
            }
            let (mut ledger, next_cursor) = self.load_scoped(hostel, &refs).await?;
            let outcome = apply_batch(&mut ledger, hostel, &request.commands, next_cursor);
            if !outcome.changes.is_empty() {
                self.write(hostel, &outcome).await?;
            }
            Ok(outcome.results)
        }
        .await;
        let end = if result.is_ok() { "COMMIT" } else { "ROLLBACK" };
        self.client.batch_execute(end).await.map_err(db)?;
        Ok(PushResponse { results: result? })
    }

    async fn pull(
        &self,
        refresh_token: &str,
        request: PullRequest,
    ) -> Result<PullResponse, ApiError> {
        let org_id = self.device_org(refresh_token).await?;
        let hostel = request.hostel_public_id.as_str();
        let owner: Option<String> = self
            .client
            .query_opt(
                "SELECT org_id FROM hostels WHERE public_id = $1",
                &[&hostel],
            )
            .await
            .map_err(db)?
            .map(|row| row.get(0));
        match owner {
            None => {
                return Ok(PullResponse {
                    cursor: request.cursor,
                    changes: Vec::new(),
                    has_more: false,
                    lease: None,
                })
            }
            Some(owner) if owner != org_id => return Err(ApiError::Unauthorized),
            Some(_) => {}
        }
        let rows = self
            .client
            .query(
                "SELECT cursor, body::text FROM changes WHERE hostel_public_id = $1 \
                 AND cursor > $2 ORDER BY cursor LIMIT $3",
                &[&hostel, &request.cursor, &(PULL_PAGE_SIZE + 1)],
            )
            .await
            .map_err(db)?;
        let has_more = rows.len() > PULL_PAGE_SIZE as usize;
        let mut cursor = request.cursor;
        let mut changes = Vec::with_capacity(rows.len().min(PULL_PAGE_SIZE as usize));
        for row in rows.iter().take(PULL_PAGE_SIZE as usize) {
            cursor = row.get(0);
            changes.push(
                serde_json::from_str::<Entity>(row.get::<_, &str>(1))
                    .map_err(|e| ApiError::Storage(e.to_string()))?,
            );
        }
        let lease = match &self.signing_key {
            Some(key) => {
                let license = self
                    .client
                    .query_one(
                        "SELECT o.licensed, to_char(current_date + 14, 'YYYY-MM-DD'), \
                         coalesce((SELECT body->>'name' FROM entities \
                                   WHERE hostel_public_id = $2 AND kind = 'hostel'), 'HostelDesk') \
                         FROM organizations o WHERE o.id = $1",
                        &[&org_id, &hostel],
                    )
                    .await
                    .map_err(db)?;
                let (licensed, expiry, name): (bool, String, String) =
                    (license.get(0), license.get(1), license.get(2));
                licensed
                    .then(|| chrono::NaiveDate::parse_from_str(&expiry, "%Y-%m-%d").ok())
                    .flatten()
                    .and_then(|expiry| sign_lease(&name, expiry, key))
            }
            None => None,
        };
        Ok(PullResponse {
            cursor,
            changes,
            has_more,
            lease,
        })
    }
}
