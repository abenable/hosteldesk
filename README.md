# HostelDesk

A local-first desktop application for hostel managers. Handles rooms, residents, check-ins, payments, and PDF receipts. Built with a Rust (Tauri v2) backend and a React + TypeScript + Tailwind frontend. Data stays in a local SQLite database; receipts are generated as offline PDFs.

## Features

- Dashboard with occupancy, revenue, outstanding balances, and recent activity
- Rooms with dual rates (semester and recess) and per-term allocations (`semester_1`, `semester_2`, `recess`) with academic year labels
- Resident directory with search, detail drawer, check-in/check-out, balances, and next-of-kin records
- Payments ledger with filters, receipt numbering, and printable PDF receipts (amount in words, hostel branding, logo, footer)
- Team management with email-based login and backend-enforced sessions
- Backup and restore of the database
- Per-hostel preferences: date format, language, receipt footer, currency (defaults to UGX)
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

- Rust (stable) with the MSVC toolchain on Windows
- Node.js 20+ and npm
- Linux only: WebKitGTK and related system packages:

  ```sh
  sudo apt update
  sudo apt install libwebkit2gtk-4.1-dev libsoup-3.0-dev libjavascriptcoregtk-4.1-dev \
    build-essential curl wget libssl-dev libgtk-3-dev libayatana-appindicator3-dev
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

- **Currency** — monetary values are always `rust_decimal::Decimal` in Rust and BigInt-based decimal strings in the frontend. Floating-point types are forbidden for money. SQLite stores money as TEXT with GLOB CHECK constraints.
- **Migrations** — live in `crates/db/migrations/` and are applied by `sqlx::migrate!`. Append-only once applied to any real database; fix forward with new timestamped files. See `AGENTS.md` for the full policy.
- **Verification** — before pushing, run:

  ```sh
  cargo check
  cargo clippy
  cargo test
  npm run lint   # from frontend/
  ```

## Licensing

HostelDesk is license-gated. On startup the app verifies an Ed25519-signed license and shows an activation screen when it is missing, invalid, or expired. Verification is fully offline.

- The public key is baked into `crates/license/src/lib.rs`; the production signing key is kept outside the repository.
- The wire format is `base64(signature):base64(json_payload)`, where the payload is:

  ```json
  {
    "hostel_name": "Ocean View Hostel",
    "expiry_date": "2099-12-31",
    "tier": "pro"
  }
  ```

- Debug builds additionally accept a development-only key, so contributors can generate local licenses without the production key: `cargo run --example generate_dev_license -p license`.

## Releasing

1. Bump `version` in `crates/app/tauri.conf.json`.
2. Tag and push:

   ```sh
   git tag v0.1.0
   git push origin main v0.1.0
   ```

3. The release workflow builds and publishes:
   - `deb` bundle from an Ubuntu 22.04 runner
   - NSIS `.exe` and MSI from a Windows runner

## Project structure

```
crates/          Rust workspace (app, core, db, pdf, license)
frontend/        React + TypeScript + Tailwind frontend (Vite)
scripts/         install.ps1 (Windows one-liner), install-dev-desktop.sh
.github/         Release workflow (tag-triggered)
specs/           Design specs and planning documents
```
