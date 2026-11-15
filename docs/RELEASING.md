# Releasing

1. Bump `version` under `[workspace.package]` in the root `Cargo.toml` (every crate inherits it), then run `cargo check` so `Cargo.lock` picks it up. If the release needs Worker changes, deploy the Worker first (`scripts/deploy-worker.sh`, see `DEPLOY.md`).
2. Tag and push:

   ```sh
   git tag v1.0.0
   git push origin main v1.0.0
   ```

3. The release workflow (`.github/workflows/release.yml`) creates one draft release, builds both platforms in parallel against it, and only un-drafts the release once **both** legs succeed — a failed Windows build leaves nothing public instead of shipping a Linux-only release. It publishes:
   - a `.deb` built with `packaging/linux/build-deb.sh` on Ubuntu 22.04, plus `checksums-linux.txt`
   - an NSIS `.exe` built from `packaging/windows/hosteldesk.nsi` on Windows, plus `checksums-windows.txt` (which `install.ps1` downloads and verifies before running the installer)
