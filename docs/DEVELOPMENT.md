# Development

## Prerequisites

- Rust (stable) with the MSVC toolchain on Windows — see `rust-toolchain.toml` for the exact version this repo builds with
- Node.js `^20.19.0 || >=22.12.0` and npm (vite 8 and oxlint both require this range; Node 20.0–20.18 and 21.x will not work)
- `cargo install tauri-cli --version "^2"`, so the `cargo tauri` subcommand used below exists
- Linux only: WebKitGTK and related system packages:

  ```sh
  sudo apt update
  sudo apt install libwebkit2gtk-4.1-dev libsoup-3.0-dev libjavascriptcoregtk-4.1-dev \
    build-essential curl wget libssl-dev libgtk-3-dev libayatana-appindicator3-dev \
    librsvg2-dev patchelf
  ```

## Run and build

```sh
cd frontend && npm install && cd ..   # install frontend dependencies
cargo tauri dev                        # run in development mode (hot reload)
cargo tauri build                      # build a release bundle for the current platform
```

On Linux, `scripts/install-dev-desktop.sh` installs a `.desktop` entry so the taskbar icon resolves during development.

## Cargo workspace

| Crate | Purpose |
|---|---|
| `crates/app` | Tauri application shell, sessions, and command layer |
| `crates/core` (`hosteldesk-core`) | Currency parsing and pure domain helpers |
| `crates/db` | SQLite schema, migrations, and repositories (sqlx) |
| `crates/pdf` | PDF receipt generation (printpdf) |
| `crates/license` | Ed25519 offline license verification |

## Conventions

- **Currency** — monetary values are `rust_decimal::Decimal` for in-Rust arithmetic, but cross a language or process boundary (SQLite storage, Tauri IPC, the frontend) as a decimal string, matched by BigInt-based helpers in the frontend. Floating-point types are forbidden for money anywhere. SQLite stores money as TEXT with GLOB CHECK constraints.
- **Migrations** — live in `crates/db/migrations/` and are applied by `sqlx::migrate!`. Append-only once applied to any real database; fix forward with new timestamped files. See `AGENTS.md` for the full policy.
- **Verification** — before pushing, run `cargo check`, `cargo clippy`, `cargo test`, and `npm run lint` (from `frontend/`).

## Project structure

```
crates/          Rust workspace (app, core, db, pdf, license)
frontend/        React + TypeScript + Tailwind frontend (Vite)
scripts/         install.ps1 (Windows one-liner), install-dev-desktop.sh
.github/         Release workflow (tag-triggered)
specs/           Design specs and planning documents (gitignored — local only, not in a fresh clone)
```
