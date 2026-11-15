-- HostelDesk sync server schema (Neon Postgres, reached through Hyperdrive).
-- Applied by scripts/migrate-sync-db.sh. Append-only once applied: fix forward
-- with a new numbered file, never edit this one.

CREATE TABLE organizations (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    -- Set to true by hand when the customer pays; only then do pulls carry a lease.
    licensed BOOLEAN NOT NULL DEFAULT false,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE accounts (
    email TEXT PRIMARY KEY,
    org_id TEXT NOT NULL REFERENCES organizations(id),
    password_hash TEXT NOT NULL,
    failed_logins INTEGER NOT NULL DEFAULT 0,
    locked_until TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX idx_accounts_org ON accounts(org_id);

CREATE TABLE sessions (
    token_hash TEXT PRIMARY KEY,
    org_id TEXT NOT NULL REFERENCES organizations(id),
    expires_at TIMESTAMPTZ NOT NULL
);

CREATE TABLE devices (
    token_hash TEXT PRIMARY KEY,
    device_id TEXT NOT NULL UNIQUE,
    org_id TEXT NOT NULL REFERENCES organizations(id),
    device_prefix TEXT NOT NULL,
    name TEXT NOT NULL,
    revoked_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (org_id, device_prefix)
);

CREATE TABLE hostels (
    public_id TEXT PRIMARY KEY,
    org_id TEXT NOT NULL REFERENCES organizations(id)
);
CREATE INDEX idx_hostels_org ON hostels(org_id);

-- Current state of every synced row, as a serialized protocol::Entity.
-- lookup_key: room number (room), receipt number (receipt), room id (allocation).
CREATE TABLE entities (
    hostel_public_id TEXT NOT NULL REFERENCES hostels(public_id),
    kind TEXT NOT NULL,
    public_id TEXT NOT NULL,
    lookup_key TEXT,
    active BOOLEAN NOT NULL DEFAULT false,
    body JSONB NOT NULL,
    PRIMARY KEY (hostel_public_id, kind, public_id)
);
CREATE INDEX idx_entities_lookup ON entities(hostel_public_id, kind, lookup_key);

CREATE TABLE applied_commands (
    hostel_public_id TEXT NOT NULL REFERENCES hostels(public_id),
    idempotency_key TEXT NOT NULL,
    PRIMARY KEY (hostel_public_id, idempotency_key)
);

-- Append-only change log that desks pull, in cursor order.
CREATE TABLE changes (
    hostel_public_id TEXT NOT NULL REFERENCES hostels(public_id),
    cursor BIGINT NOT NULL,
    body JSONB NOT NULL,
    PRIMARY KEY (hostel_public_id, cursor)
);
