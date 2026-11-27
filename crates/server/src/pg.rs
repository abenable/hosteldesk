//! Cloudflare Worker entry and the Postgres-backed [`SyncStore`].
//!
//! The database is Neon Postgres, reached through a Cloudflare Hyperdrive
//! binding (`HYPERDRIVE`), which pools connections close to Neon. Each request
//! opens one `tokio-postgres` client over a Hyperdrive socket.
//!
//! Every query uses the `*_typed` calls: one unnamed Parse/Bind/Execute per
//! query. Named prepared statements (plain `query`) break under Hyperdrive's
//! pooling, because the prepare and the execute can reach different Neon
//! connections.
//!
//! A push runs in one transaction that first locks the hostel row
//! (`SELECT … FOR UPDATE`), so two desks pushing to the same hostel apply one
//! after the other against current data.

use crate::{
    apply_batch, dump_from, entity_key, hash_password, normalize_email, random_device_prefix,
    random_id, route, token_hash, verify_password, ApiError, BatchOutcome, SyncStore, DUMMY_HASH,
    TRIAL_DAYS,
};
use protocol::{
    AuthResponse, BatchRefs, EnrollRequest, EnrollResponse, Entity, Ledger, LicenseStatus,
    PullRequest, PullResponse, PushRequest, PushResponse, SignInRequest, SignUpRequest,
    PULL_PAGE_SIZE,
};
use std::collections::{HashMap, HashSet};
use std::str::FromStr;
use tokio_postgres::error::SqlState;
use tokio_postgres::types::{ToSql, Type};
use tokio_postgres::Client;
use worker::postgres_tls::PassthroughTls;
use worker::{event, Context, Env, Request, Response, SecureTransport, Socket};

/// One typed query parameter.
type Param<'a> = (&'a (dyn ToSql + Sync), Type);

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
    let authorization = request.headers().get("authorization")?;
    let body = request.bytes().await?;
    let client = match connect(&env).await {
        Ok(client) => client,
        Err(error) => {
            worker::console_error!("database connection failed: {error}");
            return text_response(503, "database unavailable, try again".into());
        }
    };
    let store = PgStore { client };
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
    // StartTls lets `PassthroughTls` upgrade the socket if tokio-postgres
    // asks for TLS (the Hyperdrive connection string disables it today).
    let socket = Socket::builder()
        .secure_transport(SecureTransport::StartTls)
        .connect(hyperdrive.host(), hyperdrive.port())
        .map_err(|e| e.to_string())?;
    let (client, connection) = config
        .connect_raw(socket, PassthroughTls)
        .await
        .map_err(|e| e.to_string())?;
    worker::wasm_bindgen_futures::spawn_local(async move {
        if let Err(error) = connection.await {
            worker::console_error!("postgres connection error: {error:?}");
        }
    });
    Ok(client)
}

struct PgStore {
    client: Client,
}

fn db(error: tokio_postgres::Error) -> ApiError {
    ApiError::Storage(format!("{error:?}"))
}

fn is_unique_violation(error: &tokio_postgres::Error) -> bool {
    error.code() == Some(&SqlState::UNIQUE_VIOLATION)
}

fn list(set: &HashSet<String>) -> Vec<&str> {
    set.iter().map(String::as_str).collect()
}

impl PgStore {
    async fn rows(
        &self,
        sql: &str,
        params: &[Param<'_>],
    ) -> Result<Vec<tokio_postgres::Row>, ApiError> {
        self.client.query_typed(sql, params).await.map_err(db)
    }

    async fn row(&self, sql: &str, params: &[Param<'_>]) -> Result<tokio_postgres::Row, ApiError> {
        self.client.query_typed_one(sql, params).await.map_err(db)
    }

    async fn maybe_row(
        &self,
        sql: &str,
        params: &[Param<'_>],
    ) -> Result<Option<tokio_postgres::Row>, ApiError> {
        self.client.query_typed_opt(sql, params).await.map_err(db)
    }

    async fn exec(&self, sql: &str, params: &[Param<'_>]) -> Result<u64, tokio_postgres::Error> {
        self.client.execute_typed(sql, params).await
    }

    /// Rows of `SELECT body::text …`, parsed back into entities.
    async fn bodies(&self, sql: &str, params: &[Param<'_>]) -> Result<Vec<Entity>, ApiError> {
        self.rows(sql, params)
            .await?
            .iter()
            .map(|row| {
                serde_json::from_str::<Entity>(row.get::<_, &str>(0))
                    .map_err(|e| ApiError::Storage(e.to_string()))
            })
            .collect()
    }

    async fn device_org(&self, refresh_token: &str) -> Result<String, ApiError> {
        self.maybe_row(
            "SELECT org_id FROM devices WHERE token_hash = $1 AND revoked_at IS NULL",
            &[(&token_hash(refresh_token), Type::TEXT)],
        )
        .await?
        .map(|row| row.get(0))
        .ok_or(ApiError::Unauthorized)
    }

    /// The device's organization, refused while the account awaits approval.
    async fn approved_org(&self, refresh_token: &str) -> Result<String, ApiError> {
        let row = self
            .maybe_row(
                "SELECT d.org_id, o.approved_at IS NOT NULL FROM devices d \
                 JOIN organizations o ON o.id = d.org_id \
                 WHERE d.token_hash = $1 AND d.revoked_at IS NULL",
                &[(&token_hash(refresh_token), Type::TEXT)],
            )
            .await?
            .ok_or(ApiError::Unauthorized)?;
        if row.get::<_, bool>(1) {
            Ok(row.get(0))
        } else {
            Err(ApiError::NotApproved)
        }
    }

    async fn new_session(&self, org_id: &str) -> Result<String, ApiError> {
        let access_token = random_id()?;
        self.exec(
            "INSERT INTO sessions (token_hash, org_id, expires_at) \
             VALUES ($1, $2, now() + interval '30 days')",
            &[
                (&token_hash(&access_token), Type::TEXT),
                (&org_id, Type::TEXT),
            ],
        )
        .await
        .map_err(db)?;
        Ok(access_token)
    }

    /// Read only the rows `refs` needs. Runs inside the push's `BEGIN`.
    async fn load_scoped(&self, hostel: &str, refs: &BatchRefs) -> Result<(Ledger, i64), ApiError> {
        let h: Param = (&hostel, Type::TEXT);
        let mut entities = self
            .bodies(
                "SELECT body::text FROM entities WHERE hostel_public_id = $1 \
                 AND kind IN ('hostel', 'preferences')",
                std::slice::from_ref(&h),
            )
            .await?;
        for (kind, ids) in [
            ("allocation", &refs.allocations),
            ("resident", &refs.residents),
            ("payment", &refs.payments),
        ] {
            entities.extend(
                self.bodies(
                    "SELECT body::text FROM entities WHERE hostel_public_id = $1 \
                     AND kind = $2 AND public_id = ANY($3)",
                    &[
                        h.clone(),
                        (&kind, Type::TEXT),
                        (&list(ids), Type::TEXT_ARRAY),
                    ],
                )
                .await?,
            );
        }
        entities.extend(
            self.bodies(
                "SELECT body::text FROM entities WHERE hostel_public_id = $1 \
                 AND kind = 'receipt' AND lookup_key = ANY($2)",
                &[h.clone(), (&list(&refs.receipt_numbers), Type::TEXT_ARRAY)],
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
            self.bodies(
                "SELECT body::text FROM entities WHERE hostel_public_id = $1 AND kind = 'room' \
                 AND (public_id = ANY($2) OR lookup_key = ANY($3))",
                &[
                    h.clone(),
                    (&list(&rooms), Type::TEXT_ARRAY),
                    (&list(&refs.room_numbers), Type::TEXT_ARRAY),
                ],
            )
            .await?,
        );
        // A stay can come back twice (named, and active in a named room);
        // `Ledger::from_dump` keys rows by id, so that is harmless.
        entities.extend(
            self.bodies(
                "SELECT body::text FROM entities WHERE hostel_public_id = $1 \
                 AND kind = 'allocation' AND active AND lookup_key = ANY($2)",
                &[h.clone(), (&list(&rooms), Type::TEXT_ARRAY)],
            )
            .await?,
        );
        let applied = self
            .rows(
                "SELECT idempotency_key FROM applied_commands \
                 WHERE hostel_public_id = $1 AND idempotency_key = ANY($2)",
                &[h.clone(), (&list(&refs.idempotency_keys), Type::TEXT_ARRAY)],
            )
            .await?
            .iter()
            .map(|row| row.get(0))
            .collect();
        let next_cursor: i64 = self
            .row(
                "SELECT COALESCE(MAX(cursor), 0) + 1 FROM changes WHERE hostel_public_id = $1",
                &[h],
            )
            .await?
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
        let h: Param = (&hostel, Type::TEXT);
        self.exec(
            "INSERT INTO entities (hostel_public_id, kind, public_id, lookup_key, active, body) \
             SELECT $1, k, p, l, a, b::jsonb \
             FROM unnest($2::text[], $3::text[], $4::text[], $5::bool[], $6::text[]) AS t(k, p, l, a, b) \
             ON CONFLICT (hostel_public_id, kind, public_id) DO UPDATE SET \
                 lookup_key = excluded.lookup_key, active = excluded.active, body = excluded.body",
            &[
                h.clone(),
                (&kinds, Type::TEXT_ARRAY),
                (&ids, Type::TEXT_ARRAY),
                (&keys, Type::TEXT_ARRAY),
                (&actives, Type::BOOL_ARRAY),
                (&entity_bodies, Type::TEXT_ARRAY),
            ],
        )
        .await
        .map_err(db)?;
        self.exec(
            "INSERT INTO applied_commands (hostel_public_id, idempotency_key) \
             SELECT $1, unnest($2::text[])",
            &[h.clone(), (&outcome.applied, Type::TEXT_ARRAY)],
        )
        .await
        .map_err(db)?;
        self.exec(
            "INSERT INTO changes (hostel_public_id, cursor, body) \
             SELECT $1, c, b::jsonb FROM unnest($2::int8[], $3::text[]) AS t(c, b)",
            &[
                h,
                (&cursors, Type::INT8_ARRAY),
                (&change_bodies, Type::TEXT_ARRAY),
            ],
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
            .maybe_row(
                "SELECT 1 FROM accounts WHERE email = $1",
                &[(&email, Type::TEXT)],
            )
            .await?;
        if taken.is_some() {
            return Err(ApiError::BadRequest("email is already registered".into()));
        }
        let password_hash = hash_password(&request.password)?;
        let org_id = random_id()?;
        let access_token = random_id()?;
        // One statement, so the organization, account and first session are
        // created together or not at all.
        let result = self
            .exec(
                "WITH org AS (INSERT INTO organizations (id, name, license_plan, license_expires_on) \
                              VALUES ($1, $2, 'trial', current_date + $6::int)), \
                      acc AS (INSERT INTO accounts (email, org_id, password_hash) \
                              VALUES ($3, $1, $4)) \
                 INSERT INTO sessions (token_hash, org_id, expires_at) \
                 VALUES ($5, $1, now() + interval '30 days')",
                &[
                    (&org_id, Type::TEXT),
                    (&request.organization_name.trim(), Type::TEXT),
                    (&email, Type::TEXT),
                    (&password_hash, Type::TEXT),
                    (&token_hash(&access_token), Type::TEXT),
                    (&(TRIAL_DAYS as i32), Type::INT4),
                ],
            )
            .await;
        match result {
            Err(error) if is_unique_violation(&error) => {
                Err(ApiError::BadRequest("email is already registered".into()))
            }
            Err(error) => Err(db(error)),
            Ok(_) => Ok(AuthResponse {
                access_token,
                organization_id: org_id,
            }),
        }
    }

    async fn sign_in(&self, request: SignInRequest) -> Result<AuthResponse, ApiError> {
        let email = normalize_email(&request.email);
        let account = self
            .maybe_row(
                "SELECT org_id, password_hash, coalesce(locked_until > now(), false) \
                 FROM accounts WHERE email = $1",
                &[(&email, Type::TEXT)],
            )
            .await?;
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
            self.exec(
                "UPDATE accounts SET \
                 failed_logins = CASE WHEN failed_logins + 1 >= 5 THEN 0 ELSE failed_logins + 1 END, \
                 locked_until = CASE WHEN failed_logins + 1 >= 5 \
                     THEN now() + interval '15 minutes' ELSE locked_until END \
                 WHERE email = $1",
                &[(&email, Type::TEXT)],
            )
            .await
            .map_err(db)?;
            return Err(ApiError::Unauthorized);
        }
        self.exec(
            "UPDATE accounts SET failed_logins = 0, locked_until = NULL WHERE email = $1",
            &[(&email, Type::TEXT)],
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
            .maybe_row(
                "SELECT org_id FROM sessions WHERE token_hash = $1 AND expires_at > now()",
                &[(&token_hash(access_token), Type::TEXT)],
            )
            .await?
            .map(|row| row.get(0))
            .ok_or(ApiError::Unauthorized)?;
        let name: String = request.device_name.trim().chars().take(100).collect();
        let mut attempt = 0;
        let (device_id, device_prefix, refresh_token) = loop {
            let (device_id, device_prefix, refresh_token) =
                (random_id()?, random_device_prefix()?, random_id()?);
            let inserted = self
                .exec(
                    "INSERT INTO devices (token_hash, device_id, org_id, device_prefix, name) \
                     VALUES ($1, $2, $3, $4, $5)",
                    &[
                        (&token_hash(&refresh_token), Type::TEXT),
                        (&device_id, Type::TEXT),
                        (&org_id, Type::TEXT),
                        (&device_prefix, Type::TEXT),
                        (&name, Type::TEXT),
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
            .rows(
                "SELECT public_id FROM hostels WHERE org_id = $1 ORDER BY public_id",
                &[(&org_id, Type::TEXT)],
            )
            .await?
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
        let org_id = self.approved_org(refresh_token).await?;
        let refs = BatchRefs::of(&request.commands);
        let hostel = request.hostel_public_id.as_str();
        // `Client::transaction` needs `&mut`. This client is this request's
        // own connection, and Hyperdrive pins it to one Neon connection from
        // BEGIN to COMMIT, so a plain BEGIN … COMMIT is the same thing.
        self.client.batch_execute("BEGIN").await.map_err(db)?;
        let result = async {
            self.exec(
                "INSERT INTO hostels (public_id, org_id) VALUES ($1, $2) \
                 ON CONFLICT (public_id) DO NOTHING",
                &[(&hostel, Type::TEXT), (&org_id, Type::TEXT)],
            )
            .await
            .map_err(db)?;
            let owner: String = self
                .row(
                    "SELECT org_id FROM hostels WHERE public_id = $1 FOR UPDATE",
                    &[(&hostel, Type::TEXT)],
                )
                .await?
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
        let org_id = self.approved_org(refresh_token).await?;
        let hostel = request.hostel_public_id.as_str();
        let owner: Option<String> = self
            .maybe_row(
                "SELECT org_id FROM hostels WHERE public_id = $1",
                &[(&hostel, Type::TEXT)],
            )
            .await?
            .map(|row| row.get(0));
        match owner {
            None => {
                return Ok(PullResponse {
                    cursor: request.cursor,
                    changes: Vec::new(),
                    has_more: false,
                })
            }
            Some(owner) if owner != org_id => return Err(ApiError::Unauthorized),
            Some(_) => {}
        }
        let rows = self
            .rows(
                "SELECT cursor, body::text FROM changes WHERE hostel_public_id = $1 \
                 AND cursor > $2 ORDER BY cursor LIMIT $3",
                &[
                    (&hostel, Type::TEXT),
                    (&request.cursor, Type::INT8),
                    (&(PULL_PAGE_SIZE + 1), Type::INT8),
                ],
            )
            .await?;
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
        Ok(PullResponse {
            cursor,
            changes,
            has_more,
        })
    }

    async fn license(&self, refresh_token: &str) -> Result<LicenseStatus, ApiError> {
        let org_id = self.device_org(refresh_token).await?;
        let row = self
            .row(
                "SELECT name, license_plan, to_char(license_expires_on, 'YYYY-MM-DD'), \
                 approved_at IS NOT NULL FROM organizations WHERE id = $1",
                &[(&org_id, Type::TEXT)],
            )
            .await?;
        let expires_on: String = row.get(2);
        Ok(LicenseStatus {
            account_name: row.get(0),
            plan: row.get(1),
            expires_on: chrono::NaiveDate::parse_from_str(&expires_on, "%Y-%m-%d")
                .map_err(|e| ApiError::Storage(e.to_string()))?,
            approved: row.get(3),
        })
    }
}
