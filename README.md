<img src="frontend/src/assets/logo.png" alt="HostelDesk logo" width="96" />

# HostelDesk

A local-first desktop application for hostel managers. Handles rooms, residents, check-ins, payments, and PDF receipts. Built with a Rust (Tauri v2) backend and a React + TypeScript + Tailwind frontend. Data stays in a local SQLite database; receipts are generated as offline PDFs.

## Features

- Dashboard with occupancy, revenue, outstanding balances, and recent activity
- Rooms with dual rates (semester and recess) and per-term allocations (`semester_1`, `semester_2`, `recess`) with academic year labels
- Resident directory with search, detail drawer, check-in/check-out, balances, and next-of-kin records
- Payments ledger with filters, receipt numbering, and printable PDF receipts (amount in words, hostel branding, logo, footer)
- Team management with email-based login and backend-enforced sessions
- Backup and restore of the database
- Per-hostel preferences: receipt footer, currency (defaults to UGX). A date format and language preference are also stored, but nothing currently reads them — every date renders as `en-GB` and the UI/DB only offer English regardless of the stored value.
- Offline license activation (Ed25519-signed keys, no network required)

## Install

### Windows (10 and newer)

Three options, all built automatically on each `v*` release:

1. **Setup installer (.exe)** — download `HostelDesk_<version>_x64-setup.exe` from [Releases](https://github.com/abenable/hosteldesk/releases) and run it. Per-user install, no admin required.
2. **MSI package (.msi)** — download `HostelDesk_<version>_x64_en-US.msi` for machine-wide installs or IT deployment. Silent: `msiexec /i HostelDesk_<version>_x64_en-US.msi /qn`.
3. **PowerShell one-liner** — resolves and installs the latest release automatically:

   ```powershell
   powershell -c "irm https://raw.githubusercontent.com/abenable/hosteldesk/main/scripts/install.ps1|iex"
   ```

### Linux (Ubuntu 22.04 and newer)

Download `HostelDesk_<version>_amd64.deb` from [Releases](https://github.com/abenable/hosteldesk/releases) and install with `sudo apt install ./HostelDesk_<version>_amd64.deb`. Runtime dependencies (WebKitGTK and friends) are declared by the package.

## Development

### Prerequisites

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

### Run and build

```sh
# Install frontend dependencies
cd frontend && npm install && cd ..

# Run in development mode (hot reload, from the workspace root)
cargo tauri dev

# Build a release bundle for the current platform
cargo tauri build
```

On Linux, `scripts/install-dev-desktop.sh` installs a `.desktop` entry so the taskbar icon resolves during development.

### Cargo workspace

| Crate | Purpose |
|---|---|
| `crates/app` | Tauri application shell, sessions, and command layer |
| `crates/core` (`hosteldesk-core`) | Currency parsing and pure domain helpers |
| `crates/db` | SQLite schema, migrations, and repositories (sqlx) |
| `crates/pdf` | PDF receipt generation (printpdf) |
| `crates/license` | Ed25519 offline license verification |

### Conventions

- **Currency** — monetary values are `rust_decimal::Decimal` for in-Rust arithmetic, but cross a language or process boundary (SQLite storage, Tauri IPC, the frontend) as a decimal string, matched by BigInt-based helpers in the frontend. Floating-point types are forbidden for money anywhere. SQLite stores money as TEXT with GLOB CHECK constraints.
- **Migrations** — live in `crates/db/migrations/` and are applied by `sqlx::migrate!`. Append-only once applied to any real database; fix forward with new timestamped files. See `AGENTS.md` for the full policy.
- **Verification** — before pushing, run:

  ```sh
  cargo check
  cargo clippy
  cargo test
  npm run lint   # from frontend/
  ```

## Licensing

HostelDesk is proprietary, closed-source software — see [`LICENSE`](LICENSE) for the source code's terms and [`EULA.md`](EULA.md) for the end-user terms governing the compiled application. Contact ableabenaitwe@gmail.com for licensing, purchasing, or support questions.

The app itself is license-gated. On startup it verifies an Ed25519-signed license and shows an activation screen when it is missing, invalid, or expired. Verification is fully offline.

- The public key is baked into `crates/license/src/lib.rs`; the matching production signing key lives at `~/.hosteldesk/signing_key.b64` (outside the repository, `chmod 600`) — back it up somewhere durable (a password manager), since losing it means no more licenses can ever be issued against the public key already baked into shipped builds.
- The wire format is `base64(signature):base64(json_payload)`, where the payload is:

  ```json
  {
    "hostel_name": "Ocean View Hostel",
    "expiry_date": "2099-12-31",
    "tier": "1-year"
  }
  ```

- **Plans** — `crates/license/examples/issue_license.rs` mints a license file for one of four fixed plans, each just a duration off the activation date (a semester = 6 months):

  | Plan | Duration |
  |---|---|
  | `trial` | 1 semester (6 months) |
  | `1-year` | 2 semesters (12 months) |
  | `3-year` | 36 months |
  | `5-year` | 60 months |

  Issue one with:

  ```sh
  cargo run --example issue_license -p license -- \
    --hostel-name "Ocean View Hostel" \
    --plan 1-year \
    --key-file ~/.hosteldesk/signing_key.b64
  ```

  This writes a `<slug>-<plan>.hdlicense` file — send that file to the customer; they pick it on the app's activation screen.

- Builds with the `dev-license` Cargo feature additionally accept a development-only key, so contributors can generate local licenses without the production key: `cargo run --example generate_dev_license -p license > dev.hdlicense`, then run `cargo tauri dev --features dev-license` (or `cargo tauri build --debug --features dev-license` for a debug bundle) and pick `dev.hdlicense` on the activation screen. The feature is off by default — including for a plain `cargo tauri build --debug` — so a shippable build can never accept a dev-minted license.
- Activation is file-based only: the app opens a native file picker and reads the selected `.hdlicense`/`.lic`/`.txt` file — there is no paste-a-key text field.

## Releasing

1. Bump `version` in `crates/app/tauri.conf.json`, every `crates/*/Cargo.toml`, and `frontend/package.json` to the same value, then run `cargo check` so `Cargo.lock` picks it up.
2. Tag and push:

   ```sh
   git tag v1.0.0
   git push origin main v1.0.0
   ```

3. The release workflow (`.github/workflows/release.yml`) creates one draft release, builds both platforms in parallel against it, and only un-drafts the release once **both** legs succeed — a failed Windows build leaves nothing public instead of shipping a Linux-only release. It publishes:
   - `deb` bundle from an Ubuntu 22.04 runner, with `checksums-linux.txt`
   - NSIS `.exe` and MSI from a Windows runner, with `checksums-windows.txt` (which `install.ps1` downloads and verifies before running the installer)

## Project structure

```
crates/          Rust workspace (app, core, db, pdf, license)
frontend/        React + TypeScript + Tailwind frontend (Vite)
scripts/         install.ps1 (Windows one-liner), install-dev-desktop.sh
.github/         Release workflow (tag-triggered)
specs/           Design specs and planning documents (gitignored — local only, not in a fresh clone)
```
