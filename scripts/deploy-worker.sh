#!/usr/bin/env bash
# Deploy the sync Worker: apply pending D1 migrations, then publish the Worker.
# One-time setup (Workers Paid plan, `wrangler login`, `d1 create`, secrets): docs/DEPLOY.md.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/../crates/server"
npx -y wrangler@4 d1 migrations apply hosteldesk --remote
npx -y wrangler@4 deploy
