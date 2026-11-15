# GLOBAL SYSTEM INSTRUCTIONS FOR AGENT

## 1. Architecture

This is an all-**Rust** project: an **egui** desktop app plus a sync API that runs as a **Cloudflare Worker**, organized as a **Cargo workspace** with the following members:

- `core` — currency parsing shared between `app` and `db` (see `hosteldesk-core`)
- `db` — local SQLite ledger (sqlx), including the offline sync cache
- `pdf` — PDF generation
- `protocol` — sync wire types and the ledger rules, shared by the desktop and the server
- `app` — egui shell and the `Desk` application service
- `server` — the sync API. Built for `wasm32` it is the Cloudflare Worker (workers-rs, Neon Postgres through Hyperdrive, `crates/server/wrangler.toml`); built natively it serves an in-memory store for tests. **No web framework** (no axum/tower/hyper): routing is a `match` on method and path.

## 2. Tech Stack

- **Rust**
- **egui / eframe**
- **sqlx** (SQLite on the desktop)
- **workers-rs** + **tokio-postgres** over **Cloudflare Hyperdrive** to **Neon Postgres** (the sync Worker)
- **rust_decimal**
- **chrono**

## 3. Currency Constraint

**NEVER** use `f64` or `f32` for currency. **Always** use `rust_decimal` (`rust_decimal::Decimal`) for all monetary values, calculations, and storage. Floating-point types are forbidden anywhere a currency value is represented.

## 4. Workflow

Follow **Test-Driven Development (TDD)**:

1. Write a failing test in Rust that describes the desired behavior.
2. Run the test and confirm it fails for the expected reason.
3. Implement the minimum logic required to make the test pass.
4. Refactor with confidence, keeping tests green.

## 5. Verification

After **every significant change**, run the following and do **not** proceed if any of them produce warnings or errors:

```sh
cargo check
cargo clippy --workspace --all-targets -- -D warnings
cargo clippy -p server --target wasm32-unknown-unknown -- -D warnings
cargo test --workspace
```

When `crates/server` or `crates/protocol` changes, also run the Worker against a local Postgres and `scripts/smoke-worker.sh` (steps in `docs/DEVELOPMENT.md`).

Treat warnings as failures. Fix the root cause; do not suppress or ignore them.

## 6. Documentation

Document **all public Rust structs, enums, and functions** with doc comments (`///`). Documentation should describe purpose and behavior; non-obvious parameters and return values should be explained.

## 7. Self-Review

Before completing any task, review the code against these security and quality practices:

- **SQL injection prevention** — use `sqlx::query`/`sqlx::query_as::<_, T>` (desktop) and `tokio-postgres` `query(sql, &[..])` (Worker) with bound (`?N` / `$N`) parameters, never string-interpolated SQL. In the Worker, bind a list as one array parameter (`= ANY($N)`, `unnest($N::text[])`), not one parameter per item, and use only the `*_typed` calls (`query_typed`, `execute_typed`, …): Hyperdrive pools connections per transaction, so the named prepared statements behind plain `query`/`execute` fail intermittently in production (they work against a local Postgres, so tests will not catch it). Prefer the runtime `query_as::<_, T>` form over the `query!`/`query_as!` macros: the macros need a live `DATABASE_URL` (or a committed `.sqlx/` cache) to compile, which this project deliberately avoids so CI never needs a database just to build.
- **Error handling** — use `Result<T, E>` and propagate errors explicitly. Avoid panics in normal control flow.
- **Money on the wire** — amounts and rates are decimal strings checked with `protocol::validate_money`, which matches the desk's SQLite CHECK constraints, so nothing the server accepts can poison a desk's pull.
- **No `.unwrap()` in production code** — use `?`, `.context(...)`, `unwrap_or`, `unwrap_or_else`, or explicit `match` instead. `.unwrap()` / `.expect()` are acceptable only in tests.

## 8. Migrations

Desk migrations live in `crates/db/migrations/` and are applied by `sqlx::migrate!`. Worker (Neon Postgres) migrations live in `crates/server/migrations/` and are applied by `scripts/migrate-sync-db.sh` (run by `scripts/deploy-worker.sh`), which records them in `schema_migrations`. The rules below apply to both.

- **`20260905000000_initial_schema.sql` is frozen as of the `v1.0.0` tag.** It was re-baselined into a single file immediately before release, which is only safe because nothing had shipped yet. Once `v1.0.0` is tagged, that precondition no longer holds — never edit this file again; fix forward with a new timestamped migration, per the append-only rule below.
- Migrations are **append-only** once shipped or applied to any teammate's or user's database. Never edit a migration file that has ever run against a real DB; fix forward with a new timestamped file.
- A broken development DB can be recovered by deleting `<app-data-dir>/hosteldesk.db*` (including `-wal` and `-shm` files) and relaunching the app.
- Table-rewrite migrations must use explicit column lists: `INSERT INTO new (a, b) SELECT a, b FROM old`. Never use `SELECT *`, because column-order drift silently corrupts data.
- A migration that drops indexes must re-create the equivalent indexes at the end of the same file.
- Use `-- no-transaction` only for migrations that toggle `PRAGMA foreign_keys`; everything else runs inside sqlx's default transaction.

## 9. Branching and Releases

- Every release — **major or minor** — is developed on a **version branch** named after the upcoming release (e.g. `v1.0.1`, `v1.1.0`, `v2.0.0`). **All changes for that release go on the version branch** — not directly on `main`.
- To cut a release: bump `version` under `[workspace.package]` in the root `Cargo.toml` (every crate inherits it), merge the version branch into `main`, then tag:

  ```sh
  git tag v1.0.1
  git push origin main v1.0.1
  ```

- The tag push triggers `.github/workflows/release.yml`, which builds the Ubuntu `.deb` (22.04) and the Windows NSIS `.exe`, uploads platform checksum files, and publishes the GitHub release. CI (`.github/workflows/ci.yml`) runs fmt, clippy `-D warnings` (native and wasm32), tests, and an installer-script compile on every `main` push and PR.
- The sync Worker is deployed separately with `scripts/deploy-worker.sh` (Neon migrations, then `wrangler deploy`); see `docs/DEPLOY.md`. Deploy the Worker before releasing a desk build that depends on a new endpoint or migration.
- Do not move or re-tag a version tag once its release run has started; if a release fails, fix forward on `main` and cut the next version tag.
