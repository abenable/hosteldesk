# Releasing

1. Bump `version` in `crates/app/tauri.conf.json`, every `crates/*/Cargo.toml`, and `frontend/package.json` to the same value, then run `cargo check` so `Cargo.lock` picks it up.
2. Tag and push:

   ```sh
   git tag v1.0.0
   git push origin main v1.0.0
   ```

3. The release workflow (`.github/workflows/release.yml`) creates one draft release, builds both platforms in parallel against it, and only un-drafts the release once **both** legs succeed — a failed Windows build leaves nothing public instead of shipping a Linux-only release. It publishes:
   - `deb` bundle from an Ubuntu 22.04 runner, with `checksums-linux.txt`
   - NSIS `.exe` and MSI from a Windows runner, with `checksums-windows.txt` (which `install.ps1` downloads and verifies before running the installer)
