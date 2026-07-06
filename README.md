# HostelDesk

A local-first desktop application for hostel managers. Built with a Rust (Tauri v2) backend and a React + TypeScript + Tailwind frontend.

## Quick start

```sh
# 1. Install Rust dependencies (system packages required for Tauri v2 on Linux)
sudo apt update
sudo apt install libwebkit2gtk-4.1-dev libsoup-3.0-dev libjavascriptcoregtk-4.1-dev build-essential curl wget libssl-dev libgtk-3-dev libayatana-appindicator3-dev

# 2. Install Node dependencies
cd frontend && npm install

# 3. Run in development mode (from the workspace root)
cd ..
cargo tauri dev
```

## Cargo workspace

| Crate | Purpose |
|---|---|
| `crates/app` | Tauri application shell and command layer |
| `crates/core` (`hosteldesk-core`) | Pure domain logic and validation |
| `crates/db` | SQLite schema, migrations, and repositories |
| `crates/pdf` | PDF receipt generation |
| `crates/license` | Ed25519 offline license verification |

## Verification

```sh
cargo check
cargo clippy
cargo test
npm run lint        # run from frontend/
```

## Offline licensing

HostelDesk uses an Ed25519-signed license that is verified locally. The public key is baked into `crates/license/src/lib.rs`. The matching private key is kept outside this repository and is used to sign license payloads of the form:

```json
{
  "hostel_name": "Ocean View Hostel",
  "expiry_date": "2099-12-31",
  "tier": "pro"
}
```

The wire format is `base64(signature):base64(json_payload)`. On startup the app verifies the stored license and shows an activation screen if it is missing, invalid, or expired.

To generate a license for testing, use the private key that matches `PUBLIC_KEY_BASE64`.
