# Spec: Backend support for onboarding persistence

Make the hostel onboarding workflow persist for real. Today the frontend's
onboarding steps 1 (hostel profile) and 2 (room inventory) write to
`localStorage` because no backend commands exist. This spec adds the missing
repo functions + Tauri commands so the wizard persists to SQLite, and exposes
a derived `onboarding_complete` flag so the app can skip the wizard on
subsequent launches.

Backend only (`crates/db`, `crates/app`). A separate, small frontend rewire
follows once this lands — noted at the bottom, out of scope here.

## Read first (existing patterns to match)

- `crates/db/src/lib.rs` — `DbPool` repo impl. `get_rooms` (query_as + bind),
  `add_resident` (query + bind + `last_insert_rowid`), `create_allocation`
  (transaction, `inspect_err` logging, `info!` on success). Match this style:
  `#[instrument(skip(self, ...))]`, `debug!` on entry, `inspect_err` with
  `error!` on DB errors, `info!` with the new id on success.
- `crates/db/src/model.rs` — `Room`, `NewResident`, `NewStaffUser`, etc.
  Derives `Debug, Clone, Serialize, Deserialize`; row types also derive
  `FromRow`. Doc-comment every new public struct/field.
- `crates/app/src/commands.rs` — request structs derive `Debug, Deserialize`;
  each command `#[tauri::command]` + `#[instrument]`, parses `Decimal` via
  `Decimal::from_str`, calls repo, maps errors via `map_db_error` (or a custom
  message for the rate-parse failure, matching `create_allocation`).
- `crates/app/src/lib.rs` — `generate_handler![...]` is where new commands
  must be registered. `LicenseState` is the only non-DB state.
- `crates/db/migrations/20250706000000_initial_schema.sql` — `hostels` table
  already exists with `id, name, address, phone, currency NOT NULL DEFAULT
  'USD', branding_logo_path, created_at`. **No migration is needed for
  hostels.** The `rooms` table also already exists with the constraints below.

### Hard constraints from the schema (do not violate)

- `rooms`: `UNIQUE (hostel_id, room_number)` — a duplicate insert must surface
  as a clean error, not a panic.
- `rooms`: `CHECK (capacity > 0)` — reject `capacity <= 0` before insert (let
  the CHECK be the backstop, but validate in code for a friendly message).
- `rooms`: `CHECK (status IN ('vacant','occupied','maintenance'))` — new rooms
  are always created with `status = 'vacant'`; do not accept a status from the
  caller.
- `rooms.rate_per_period` has a decimal-GLOB `CHECK` (see migration
  `20260706000001`). Validate the rate with `Decimal::from_str` and store
  `.to_string()` — exactly the pattern `create_allocation` uses for
  `rate_agreed`. This normalizes `"5"` → `"5"` and rejects `"abc"`.
- `hostels.currency NOT NULL DEFAULT 'USD'` — caller may omit currency; repo
  should not require it.
- All money is `rust_decimal::Decimal`. Never `f64`.

## 1. `crates/db/src/model.rs` — new types

Add:

```rust
/// A hostel record. One row per licensed property.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Hostel {
    pub id: i64,
    pub name: String,
    pub address: Option<String>,
    pub phone: Option<String>,
    pub currency: String,
    pub branding_logo_path: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// Data required to create a hostel during onboarding.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewHostel {
    pub name: String,
    pub address: Option<String>,
    pub phone: Option<String>,
    pub currency: Option<String>,
    pub branding_logo_path: Option<String>,
}

/// Data required to create a single room.
///
/// `status` is deliberately absent — new rooms are always created `vacant`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewRoom {
    pub hostel_id: i64,
    pub room_number: String,
    pub room_type: String,
    pub capacity: i32,
    pub floor: Option<i32>,
    /// Nightly/period rate as a decimal string, e.g. `"50.00"`.
    pub rate_per_period: String,
}

/// Hostel profile augmented with derived onboarding state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostelProfile {
    pub id: i64,
    pub name: String,
    pub address: Option<String>,
    pub phone: Option<String>,
    pub currency: String,
    pub branding_logo_path: Option<String>,
    pub room_count: i64,
    pub admin_count: i64,
    /// True when the hostel has at least one room and one admin staff user.
    pub onboarding_complete: bool,
}
```

## 2. `crates/db/src/lib.rs` — new repo functions

Add to `impl DbPool`:

### `create_hostel(&self, hostel: NewHostel) -> anyhow::Result<Hostel>`

Insert and return the full row (so the caller learns the assigned `id` and
`created_at`). Use `query_as` with `RETURNING *` if sqlx supports it cleanly
for SQLite, otherwise `last_insert_rowid()` + a follow-up `query_as` SELECT.
Prefer the `RETURNING *` form — it's one round-trip and sqlx supports it on
modern SQLite.

```sql
INSERT INTO hostels (name, address, phone, currency, branding_logo_path)
VALUES (?1, ?2, ?3, ?4, ?5)
RETURNING id, name, address, phone, currency, branding_logo_path, created_at
```

Bind `currency` with `hostel.currency.as_deref().unwrap_or("USD")` so the
caller may omit it.

### `add_room(&self, room: NewRoom) -> anyhow::Result<i64>`

Validate `capacity > 0` (bail with `"capacity must be greater than zero"` if
not). Validate `room_type` is one of `single | double | dorm` (bail otherwise)
— this matches the frontend's onboarding options and keeps the free-text
column honest. Insert with `status = 'vacant'` hardcoded, return
`last_insert_rowid()`. The `UNIQUE (hostel_id, room_number)` violation will
surface as a sqlx error — let `map_db_error` (in the command layer) format
it, but in the repo log it via `inspect_err` like the other inserters.

```sql
INSERT INTO rooms (hostel_id, room_number, room_type, capacity, floor, rate_per_period, status)
VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'vacant')
```

### `bulk_add_rooms(&self, rooms: Vec<NewRoom>) -> anyhow::Result<Vec<i64>>`

One transaction. Insert each room via the same SQL as `add_room` (factor the
INSERT into a private helper `insert_room(tx, &room) -> Result<i64>` so both
paths share it). If any insert fails (duplicate room_number, bad rate, etc.),
roll back the whole transaction and return the error — onboarding must be
all-or-nothing. Return the list of inserted ids in order. Capacity/type
validation runs per-row inside the helper.

`ponytail:` note is not needed — this is the genuine implementation.

### `get_hostel_profile(&self, hostel_id: i64) -> anyhow::Result<Option<HostelProfile>>`

Single query joining aggregate counts:

```sql
SELECT
  h.id, h.name, h.address, h.phone, h.currency, h.branding_logo_path,
  (SELECT COUNT(*) FROM rooms r WHERE r.hostel_id = h.id) AS room_count,
  (SELECT COUNT(*) FROM staff_users s WHERE s.hostel_id = h.id AND s.role = 'admin') AS admin_count
FROM hostels h
WHERE h.id = ?1
```

Map to `HostelProfile` with `onboarding_complete = room_count > 0 &&
admin_count > 0`. Return `None` when the hostel row doesn't exist (no rows).
Use `query_as` + `fetch_optional`.

### `list_hostels(&self) -> anyhow::Result<Vec<Hostel>>`

`SELECT * FROM hostels ORDER BY id`. Needed so the app can pick the active
hostel when one exists but `get_hostel_profile` is called with a default id.
Keep it simple — single-hostel installs will have one row.

## 3. `crates/app/src/commands.rs` — new commands

Add request structs (derive `Debug, Deserialize`, doc-comment) and commands:

### `create_hostel`

```rust
#[derive(Debug, Deserialize)]
pub struct CreateHostelRequest {
    pub name: String,
    pub address: Option<String>,
    pub phone: Option<String>,
    pub currency: Option<String>,
    pub branding_logo_path: Option<String>,
}

#[tauri::command]
#[instrument(skip(state), fields(command = "create_hostel"))]
pub async fn create_hostel(request: CreateHostelRequest, state: State<'_, DbPool>) -> Result<Hostel, String>
```

Build `NewHostel`, call `state.create_hostel(...)`, `map_db_error` on
failure. Validate non-empty `name` (bail `"hostel name is required"`).

### `add_room` and `bulk_add_rooms`

```rust
#[derive(Debug, Deserialize)]
pub struct AddRoomRequest {
    pub hostel_id: i64,
    pub room_number: String,
    pub room_type: String,
    pub capacity: i32,
    pub floor: Option<i32>,
    pub rate_per_period: String,
}

#[tauri::command]
pub async fn add_room(request: AddRoomRequest, state: State<'_, DbPool>) -> Result<i64, String>

#[tauri::command]
pub async fn bulk_add_rooms(requests: Vec<AddRoomRequest>, state: State<'_, DbPool>) -> Result<Vec<i64>, String>
```

In both, parse `rate_per_period` with `Decimal::from_str` and store
`.to_string()` (mirrors `create_allocation`). Build `NewRoom`, call repo,
`map_db_error`. For `bulk_add_rooms`, map the `Vec<AddRoomRequest>` to
`Vec<NewRoom>` (parsing every rate up front — if any fails, reject the whole
batch before touching the DB).

### `get_hostel_profile`

```rust
#[tauri::command]
pub async fn get_hostel_profile(hostel_id: i64, state: State<'_, DbPool>) -> Result<Option<HostelProfile>, String>
```

Thin wrapper over the repo function.

### `list_hostels`

```rust
#[tauri::command]
pub async fn list_hostels(state: State<'_, DbPool>) -> Result<Vec<Hostel>, String>
```

## 4. `crates/app/src/lib.rs` — register commands

Add `create_hostel`, `add_room`, `bulk_add_rooms`, `get_hostel_profile`,
`list_hostels` to the existing `generate_handler![...]` list. Do not reorder
existing entries.

## 5. Doc comments + exports

- Doc-comment every new public struct, field, and function per `agents.md` §6.
- `model.rs` types are already re-exported via `pub use model::*;` — no extra
  export line needed.
- `commands.rs` request structs are private (only the command fns are
  `pub`), matching the existing `AddResidentRequest` convention.

## 6. Tests (TDD per agents.md §4)

Add to `crates/db/src/lib.rs` `#[cfg(test)] mod tests` (or a sibling test
module — match where `password_hash_and_verify_round_trip` lives):

- `create_hostel_inserts_and_returns_row` — insert, assert returned `id > 0`,
  `currency == "USD"` when omitted, `name` matches.
- `add_room_creates_vacant_room` — insert a hostel, add a room, fetch via
  `get_rooms`, assert `status == "vacant"` and fields round-trip.
- `add_room_rejects_duplicate_room_number` — second insert with same
  `(hostel_id, room_number)` errors.
- `add_room_rejects_zero_capacity` — `capacity: 0` errors with the friendly
  message, not a CHECK violation.
- `bulk_add_rooms_atomic` — batch of 2 valid + 1 duplicate → whole batch
  rolls back, `get_rooms` shows zero new rooms.
- `get_hostel_profile_onboarding_flags` — fresh hostel: `onboarding_complete
  == false`, `room_count == 0`, `admin_count == 0`. After adding a room and
  an admin staff user: `onboarding_complete == true`.

Use an in-memory or temp-file SQLite pool. The existing test module doesn't
show a pool helper — add a small `async fn test_pool() -> DbPool` that opens
`sqlite::memory:` and runs migrations. `DbPool::open` already runs
`sqlx::migrate!("./migrations")`, so pointing it at `sqlite::` memory should
work; if the migrate macro's path doesn't resolve from the test target, use a
temp file via `tempfile::NamedTempFile` (add `tempfile` to `[dev-dependencies]`
in `crates/db/Cargo.toml` if not already present — check first, don't add
duplicates).

## Verification (must pass before reporting done)

```sh
cargo check
cargo clippy --all-targets -- -D warnings
cargo test -p db
cargo test -p app
```

Treat warnings as failures (agents.md §5). Do not proceed if any produce
output.

## Out of scope (next spec, after this lands)

- Frontend rewire: swap `onboarding.ts` localStorage stubs for real
  `create_hostel` / `bulk_add_rooms` calls; replace the `localStorage`
  completion flag in `App.tsx` with an async `getHostelProfile` check so the
  wizard is skipped once the DB says `onboarding_complete`. Thread the real
  `hostel_id` (returned by `create_hostel`) through `CheckInForm` and
  `createStaffUser` instead of the hardcoded `1`. This is a small
  frontend-implementer pass — separate spec.
- Receipt PDF / payments UI redesign.
- Multi-hostel switching (the schema supports it; the UI doesn't yet).
