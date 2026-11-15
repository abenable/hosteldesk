# Development

## Prerequisites

- Rust (stable) with the MSVC toolchain on Windows — see `rust-toolchain.toml` for the exact version and the `wasm32-unknown-unknown` target this repo builds with
- For the sync Worker only: Node.js (for `npx wrangler`), `psql`, and Docker (a local Postgres)
- Linux only: GTK and the X11/Wayland libraries eframe links against:

  ```sh
  sudo apt update
  sudo apt install libgtk-3-dev libxkbcommon-dev libxcb1-dev libx11-dev libwayland-dev libssl-dev
  ```

## Run and build

```sh
cargo run -p app                       # debug window
cargo build --release -p app          # release binary at target/release/hosteldesk
```

### Sync Worker locally

```sh
docker run -d --name hd-sync-pg -e POSTGRES_PASSWORD=devpass -e POSTGRES_DB=hosteldesk -p 55432:5432 postgres:17
SYNC_DATABASE_URL=postgres://postgres:devpass@127.0.0.1:55432/hosteldesk scripts/migrate-sync-db.sh
cd crates/server
CLOUDFLARE_HYPERDRIVE_LOCAL_CONNECTION_STRING_HYPERDRIVE="postgres://postgres:devpass@127.0.0.1:55432/hosteldesk?sslmode=disable" \
  npx -y wrangler@4 dev --port 8787                       # builds with worker-build, serves the Worker
../../scripts/smoke-worker.sh http://127.0.0.1:8787       # end-to-end check against real Postgres
```

Point a debug build of the desk at it by entering `http://127.0.0.1:8787` as the cloud server (plain HTTP is only accepted for localhost, and only in debug builds).

On Linux, `scripts/install-dev-desktop.sh` installs a `.desktop` entry so the taskbar icon resolves during development. The window app id is `com.hosteldesk.app`, the same identifier the previous Tauri build used for its data directory.

## Cargo workspace

| Crate | Purpose |
|---|---|
| `crates/app` | egui shell, `Desk` service, and desktop sync client |
| `crates/core` (`hosteldesk-core`) | Currency parsing and pure domain helpers |
| `crates/db` | SQLite schema, migrations, and repositories (sqlx) |
| `crates/pdf` | PDF receipt generation (printpdf) |
| `crates/protocol` | Sync wire types and ledger rules shared by the desk and the Worker |
| `crates/server` | Sync API: Cloudflare Worker on Neon Postgres via Hyperdrive (wasm32), in-memory store for native tests |

## Conventions

- **Currency** — monetary values are `rust_decimal::Decimal` for in-Rust arithmetic. SQLite (desk) and Postgres (Worker, inside JSONB rows) store them as decimal strings; the desk enforces them with GLOB CHECK constraints and the sync protocol with `protocol::validate_money`. Floating-point types are forbidden for money anywhere.
- **Migrations** — desk: `crates/db/migrations/` via `sqlx::migrate!`; Worker: `crates/server/migrations/` via `scripts/migrate-sync-db.sh`. Append-only once applied to any real database; fix forward with new timestamped files. See `AGENTS.md` for the full policy.
- **Verification** — before pushing, run `cargo check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo clippy -p server --target wasm32-unknown-unknown -- -D warnings`, and `cargo test --workspace`.

## Project structure

```
crates/          Rust workspace (app, core, db, pdf, protocol, server)
packaging/       Linux .deb script and Windows NSIS script
scripts/         install.ps1, install-dev-desktop.sh, deploy-worker.sh, migrate-sync-db.sh, smoke-worker.sh
.github/         CI and release workflows
```
