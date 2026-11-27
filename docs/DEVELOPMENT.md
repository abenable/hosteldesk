# Development

## Prerequisites

- Rust (stable) with the MSVC toolchain on Windows — see `rust-toolchain.toml` for the exact version and the `wasm32-unknown-unknown` target this repo builds with
- Node.js LTS and npm, for the React frontend in `frontend/` (and `npx wrangler` for the sync Worker)
- Optional: the Tauri CLI, `cargo install tauri-cli --version "^2" --locked` (gives `cargo tauri dev` / `cargo tauri build`)
- For the sync Worker only: `psql` and Docker (a local Postgres)
- Linux only: WebKitGTK 4.1 and GTK 3 development packages for Tauri:

  ```sh
  sudo apt update
  sudo apt install libwebkit2gtk-4.1-dev libjavascriptcoregtk-4.1-dev libsoup-3.0-dev libgtk-3-dev librsvg2-dev libssl-dev
  ```

- Windows: the Microsoft Edge WebView2 Runtime (preinstalled on Windows 11; the installer adds it when missing)

## Run and build

`crates/app` embeds `frontend/dist` at compile time, so build the frontend once before any cargo command (`cd frontend && npm ci && npm run build`).

Dev loop, hot-reloading the UI:

```sh
cargo tauri dev                        # runs `npm run dev` in frontend/, then the app against http://localhost:5173
# or, without the Tauri CLI, in two terminals:
npm --prefix frontend run dev          # Vite dev server on :5173
cargo run -p app                       # debug window loading the dev server
```

Release build:

```sh
cargo tauri build                      # runs `npm run build`, then a release binary with the UI embedded
# or by hand:
npm --prefix frontend run build
cargo build --release -p app --features custom-protocol   # target/release/hosteldesk
```

Without `--features custom-protocol` a release binary still tries to load `http://localhost:5173` and shows a blank window; every packaging path (`packaging/`, `release.yml`) passes it.

Rust ↔ TypeScript: every command is in `crates/app/src/ipc.rs` (one thin wrapper per `Desk` method); the contract is in `specs/v2.1.0-ui-rebuild.md`. The window listens for the `sync` event the background loop emits every 30 s while the desk is enrolled.

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

On Linux, `scripts/install-dev-desktop.sh` installs a `.desktop` entry so the taskbar icon resolves during development. The window app id (WM_CLASS) is `com.hosteldesk.app`, the Tauri identifier, which also names the data directory.

## Cargo workspace

| Crate | Purpose |
|---|---|
| `crates/app` | Tauri v2 shell, IPC commands, `Desk` service, and desktop sync client |
| `crates/core` (`hosteldesk-core`) | Currency parsing and pure domain helpers |
| `crates/db` | SQLite schema, migrations, and repositories (sqlx) |
| `crates/pdf` | PDF receipt generation (printpdf) |
| `crates/protocol` | Sync wire types and ledger rules shared by the desk and the Worker |
| `crates/server` | Sync API: Cloudflare Worker on Neon Postgres via Hyperdrive (wasm32), in-memory store for native tests |

## Conventions

- **Currency** — monetary values are `rust_decimal::Decimal` for in-Rust arithmetic. SQLite (desk) and Postgres (Worker, inside JSONB rows) store them as decimal strings; the desk enforces them with GLOB CHECK constraints and the sync protocol with `protocol::validate_money`. Floating-point types are forbidden for money anywhere.
- **Migrations** — desk: `crates/db/migrations/` via `sqlx::migrate!`; Worker: `crates/server/migrations/` via `scripts/migrate-sync-db.sh`. Append-only once applied to any real database; fix forward with new timestamped files. See `AGENTS.md` for the full policy.
- **Verification** — before pushing, run `cd frontend && npm run lint && npm run build`, then `cargo check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo clippy -p server --target wasm32-unknown-unknown -- -D warnings`, and `cargo test --workspace`.

## Project structure

```
crates/          Rust workspace (app, core, db, pdf, protocol, server)
frontend/        React + TypeScript + Vite + Tailwind UI loaded by the Tauri window
packaging/       Linux .deb script and Windows NSIS script
scripts/         install.ps1, install-dev-desktop.sh, deploy-worker.sh, migrate-sync-db.sh, smoke-worker.sh
.github/         CI and release workflows
```
