//! Postgres store. Money is a `NUMERIC` column; the rest of each row is JSON
//! so the same [`protocol::Ledger`] rules run as in the in-memory store.

use crate::{
    ApiError, AuthResponse, CommandAck, EnrollRequest, EnrollResponse, PullResponse, PushRequest,
    PushResponse, SignInRequest, SignUpRequest, SyncStore,
};
use protocol::{Entity, Ledger, PaymentSnap};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::postgres::PgPoolOptions;
use sqlx::Row;
use std::str::FromStr;

/// Server process store backed by Postgres.
pub struct PostgresStore {
    pool: sqlx::PgPool,
}

#[derive(Serialize, Deserialize)]
struct ChangeRow {
    cursor: i64,
    entity: Entity,
}

impl PostgresStore {
    /// Connect and create the server tables if they are missing.
    pub async fn connect(database_url: &str) -> anyhow::Result<Self> {
        let pool = PgPoolOptions::new()
            .max_connections(5)
            .connect(database_url)
            .await?;
        for statement in SCHEMA.split(';') {
            let statement = statement.trim();
            if statement.is_empty() {
                continue;
            }
            sqlx::query(statement).execute(&pool).await?;
        }
        Ok(Self { pool })
    }
}

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS server_accounts (
    email TEXT PRIMARY KEY,
    org_id TEXT NOT NULL,
    password_hash TEXT NOT NULL,
    organization_name TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS server_sessions (
    token_hash TEXT PRIMARY KEY,
    org_id TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS server_devices (
    token_hash TEXT PRIMARY KEY,
    org_id TEXT NOT NULL,
    device_id TEXT NOT NULL,
    device_prefix TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS server_hostels (
    hostel_public_id TEXT PRIMARY KEY,
    org_id TEXT NOT NULL,
    document JSONB NOT NULL,
    next_cursor BIGINT NOT NULL,
    changes JSONB NOT NULL
);
CREATE TABLE IF NOT EXISTS server_payment_amounts (
    public_id TEXT PRIMARY KEY,
    hostel_public_id TEXT NOT NULL,
    amount NUMERIC NOT NULL
)
"#;

#[async_trait::async_trait]
impl SyncStore for PostgresStore {
    async fn sign_up(&self, request: SignUpRequest) -> Result<AuthResponse, ApiError> {
        if request.password.chars().count() < 8 {
            return Err(ApiError::BadRequest(
                "password must be at least 8 characters".into(),
            ));
        }
        let hash = crate::hash_password(&request.password)
            .map_err(|e| ApiError::Storage(e.to_string()))?;
        let org_id = crate::random_id();
        let inserted = sqlx::query(
            "INSERT INTO server_accounts (email, org_id, password_hash, organization_name) VALUES ($1, $2, $3, $4) ON CONFLICT DO NOTHING",
        )
        .bind(&request.email)
        .bind(&org_id)
        .bind(&hash)
        .bind(&request.organization_name)
        .execute(&self.pool)
        .await
        .map_err(|e| ApiError::Storage(e.to_string()))?;
        if inserted.rows_affected() == 0 {
            return Err(ApiError::BadRequest("email is already registered".into()));
        }
        self.session_for(org_id).await
    }

    async fn sign_in(&self, request: SignInRequest) -> Result<AuthResponse, ApiError> {
        let row = sqlx::query("SELECT org_id, password_hash FROM server_accounts WHERE email = $1")
            .bind(&request.email)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| ApiError::Storage(e.to_string()))?;
        let Some(row) = row else {
            let _ = crate::verify_password(&request.password, crate::DUMMY_HASH);
            return Err(ApiError::Unauthorized);
        };
        let hash: String = row
            .try_get("password_hash")
            .map_err(|e| ApiError::Storage(e.to_string()))?;
        let org_id: String = row
            .try_get("org_id")
            .map_err(|e| ApiError::Storage(e.to_string()))?;
        if !crate::verify_password(&request.password, &hash).unwrap_or(false) {
            return Err(ApiError::Unauthorized);
        }
        self.session_for(org_id).await
    }

    async fn enroll(
        &self,
        access_token: &str,
        _request: EnrollRequest,
    ) -> Result<EnrollResponse, ApiError> {
        let org_id = self.org_for_session(access_token).await?;
        let device_id = crate::random_id();
        let prefix = device_id.chars().take(4).collect::<String>().to_uppercase();
        let refresh_token = crate::random_id();
        sqlx::query(
            "INSERT INTO server_devices (token_hash, org_id, device_id, device_prefix) VALUES ($1, $2, $3, $4)",
        )
        .bind(crate::token_hash(&refresh_token))
        .bind(&org_id)
        .bind(&device_id)
        .bind(&prefix)
        .execute(&self.pool)
        .await
        .map_err(|e| ApiError::Storage(e.to_string()))?;
        let hostel_public_ids = sqlx::query_scalar(
            "SELECT hostel_public_id FROM server_hostels WHERE org_id = $1 ORDER BY hostel_public_id",
        )
        .bind(&org_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| ApiError::Storage(e.to_string()))?;
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
        let org_id = self.org_for_device(refresh_token).await?;
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| ApiError::Storage(e.to_string()))?;
        let existing = sqlx::query(
            "SELECT org_id, document, next_cursor, changes FROM server_hostels WHERE hostel_public_id = $1 FOR UPDATE",
        )
        .bind(&request.hostel_public_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| ApiError::Storage(e.to_string()))?;

        let (mut ledger, mut next_cursor, mut changes, row_org) = if let Some(row) = existing {
            let row_org: String = row
                .try_get("org_id")
                .map_err(|e| ApiError::Storage(e.to_string()))?;
            if row_org != org_id {
                return Err(ApiError::Unauthorized);
            }
            let document: serde_json::Value = row
                .try_get("document")
                .map_err(|e| ApiError::Storage(e.to_string()))?;
            let dump =
                serde_json::from_value(document).map_err(|e| ApiError::Storage(e.to_string()))?;
            let next_cursor: i64 = row
                .try_get("next_cursor")
                .map_err(|e| ApiError::Storage(e.to_string()))?;
            let changes: serde_json::Value = row
                .try_get("changes")
                .map_err(|e| ApiError::Storage(e.to_string()))?;
            let changes: Vec<ChangeRow> = serde_json::from_value(changes).unwrap_or_default();
            (Ledger::from_dump(dump), next_cursor, changes, row_org)
        } else {
            (Ledger::default(), 1, Vec::new(), org_id.clone())
        };

        let mut results = Vec::new();
        for envelope in request.commands {
            match ledger.apply(&envelope.idempotency_key, &envelope.command) {
                protocol::ApplyOutcome::Ack { changed, replay } => {
                    if !replay {
                        for entity in changed {
                            if let Entity::Payment(payment) = &entity {
                                store_amount(&mut tx, &request.hostel_public_id, payment).await?;
                            }
                            changes.push(ChangeRow {
                                cursor: next_cursor,
                                entity,
                            });
                            next_cursor += 1;
                        }
                    }
                    results.push(CommandAck {
                        idempotency_key: envelope.idempotency_key,
                        status: "acked".into(),
                        message: None,
                    });
                }
                protocol::ApplyOutcome::Conflict { message } => results.push(CommandAck {
                    idempotency_key: envelope.idempotency_key,
                    status: "conflict".into(),
                    message: Some(message),
                }),
            }
        }
        let document =
            serde_json::to_value(ledger.dump()).map_err(|e| ApiError::Storage(e.to_string()))?;
        let changes_json =
            serde_json::to_value(&changes).map_err(|e| ApiError::Storage(e.to_string()))?;
        sqlx::query(
            r#"
            INSERT INTO server_hostels (hostel_public_id, org_id, document, next_cursor, changes)
            VALUES ($1, $2, $3, $4, $5)
            ON CONFLICT (hostel_public_id) DO UPDATE SET
                document = EXCLUDED.document,
                next_cursor = EXCLUDED.next_cursor,
                changes = EXCLUDED.changes
            "#,
        )
        .bind(&request.hostel_public_id)
        .bind(&row_org)
        .bind(&document)
        .bind(next_cursor)
        .bind(&changes_json)
        .execute(&mut *tx)
        .await
        .map_err(|e| ApiError::Storage(e.to_string()))?;
        tx.commit()
            .await
            .map_err(|e| ApiError::Storage(e.to_string()))?;
        Ok(PushResponse { results })
    }

    async fn pull(
        &self,
        refresh_token: &str,
        hostel_public_id: &str,
        cursor: i64,
    ) -> Result<PullResponse, ApiError> {
        let org_id = self.org_for_device(refresh_token).await?;
        let row = sqlx::query(
            "SELECT org_id, document, next_cursor, changes FROM server_hostels WHERE hostel_public_id = $1",
        )
        .bind(hostel_public_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| ApiError::Storage(e.to_string()))?;
        let Some(row) = row else {
            return Ok(PullResponse {
                cursor,
                changes: Vec::new(),
                lease: None,
            });
        };
        let row_org: String = row
            .try_get("org_id")
            .map_err(|e| ApiError::Storage(e.to_string()))?;
        if row_org != org_id {
            return Err(ApiError::Unauthorized);
        }
        let changes: serde_json::Value = row
            .try_get("changes")
            .map_err(|e| ApiError::Storage(e.to_string()))?;
        let changes: Vec<ChangeRow> = serde_json::from_value(changes).unwrap_or_default();
        let next_cursor: i64 = row
            .try_get("next_cursor")
            .map_err(|e| ApiError::Storage(e.to_string()))?;
        let document: serde_json::Value = row
            .try_get("document")
            .map_err(|e| ApiError::Storage(e.to_string()))?;
        let dump: protocol::LedgerDump = serde_json::from_value(document).unwrap_or_default();
        let name = dump
            .hostel
            .as_ref()
            .map(|hostel| hostel.name.clone())
            .unwrap_or_else(|| "HostelDesk".to_string());
        Ok(PullResponse {
            cursor: next_cursor.saturating_sub(1).max(cursor),
            changes: changes
                .into_iter()
                .filter(|change| change.cursor > cursor)
                .map(|change| change.entity)
                .collect(),
            lease: crate::sign_lease(&name),
        })
    }
}

impl PostgresStore {
    async fn session_for(&self, org_id: String) -> Result<AuthResponse, ApiError> {
        let access_token = crate::random_id();
        sqlx::query("INSERT INTO server_sessions (token_hash, org_id) VALUES ($1, $2)")
            .bind(crate::token_hash(&access_token))
            .bind(&org_id)
            .execute(&self.pool)
            .await
            .map_err(|e| ApiError::Storage(e.to_string()))?;
        Ok(AuthResponse {
            access_token,
            organization_id: org_id,
        })
    }

    async fn org_for_session(&self, access_token: &str) -> Result<String, ApiError> {
        let org_id: Option<String> =
            sqlx::query_scalar("SELECT org_id FROM server_sessions WHERE token_hash = $1")
                .bind(crate::token_hash(access_token))
                .fetch_optional(&self.pool)
                .await
                .map_err(|e| ApiError::Storage(e.to_string()))?;
        org_id.ok_or(ApiError::Unauthorized)
    }

    async fn org_for_device(&self, refresh_token: &str) -> Result<String, ApiError> {
        let org_id: Option<String> =
            sqlx::query_scalar("SELECT org_id FROM server_devices WHERE token_hash = $1")
                .bind(crate::token_hash(refresh_token))
                .fetch_optional(&self.pool)
                .await
                .map_err(|e| ApiError::Storage(e.to_string()))?;
        org_id.ok_or(ApiError::Unauthorized)
    }
}

async fn store_amount(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    hostel_public_id: &str,
    payment: &PaymentSnap,
) -> Result<(), ApiError> {
    let amount =
        Decimal::from_str(&payment.amount).map_err(|e| ApiError::Storage(e.to_string()))?;
    sqlx::query(
        r#"
        INSERT INTO server_payment_amounts (public_id, hostel_public_id, amount)
        VALUES ($1, $2, $3)
        ON CONFLICT (public_id) DO UPDATE SET amount = EXCLUDED.amount
        "#,
    )
    .bind(&payment.public_id)
    .bind(hostel_public_id)
    .bind(amount)
    .execute(&mut **tx)
    .await
    .map_err(|e| ApiError::Storage(e.to_string()))?;
    Ok(())
}
