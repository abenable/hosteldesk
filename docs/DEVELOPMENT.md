# Development

## Prerequisites

- Rust (stable) with the MSVC toolchain on Windows — see `rust-toolchain.toml` for the exact version this repo builds with
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

On Linux, `scripts/install-dev-desktop.sh` installs a `.desktop` entry so the taskbar icon resolves during development. The window app id is `com.hosteldesk.app`, the same identifier the previous Tauri build used for its data directory.

## Cargo workspace

| Crate | Purpose |
|---|---|
| `crates/app` | egui shell, `Desk` service, and desktop sync client |
| `crates/core` (`hosteldesk-core`) | Currency parsing and pure domain helpers |
| `crates/db` | SQLite schema, migrations, and repositories (sqlx) |
| `crates/pdf` | PDF receipt generation (printpdf) |
| `crates/license` | Ed25519 offline license and lease verification |
| `crates/protocol` | Sync commands shared with the server |
| `crates/server` | Axum + Postgres multi-tenant API |

## Conventions

- **Currency** — monetary values are `rust_decimal::Decimal` for in-Rust arithmetic. SQLite stores them as decimal strings with GLOB CHECK constraints. Postgres stores them as `NUMERIC`. Floating-point types are forbidden for money anywhere.
- **Migrations** — live in `crates/db/migrations/` and are applied by `sqlx::migrate!`. Append-only once applied to any real database; fix forward with new timestamped files. See `AGENTS.md` for the full policy.
- **Verification** — before pushing, run `cargo check`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo test --workspace`.

## Project structure

```
crates/          Rust workspace (app, core, db, pdf, license, protocol, server)
packaging/       Linux .deb script and Windows NSIS script
scripts/         install.ps1 (Windows one-liner), install-dev-desktop.sh
.github/         CI and release workflows
```
