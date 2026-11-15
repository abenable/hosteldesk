#!/usr/bin/env bash
# Deploy the sync Worker: migrate the Neon database first, then publish the Worker.
# One-time setup (Workers Paid, wrangler login, Neon, Hyperdrive, secret): docs/DEPLOY.md.
set -euo pipefail
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
"$root/scripts/migrate-sync-db.sh"
cd "$root/crates/server"
npx -y wrangler@4 deploy
