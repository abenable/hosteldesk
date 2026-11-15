# Deploying the sync Worker

The sync API (`crates/server`) is a Rust Cloudflare Worker named `hosteldesk-sync`.
- **Data:** a **Neon Postgres** database (project `hosteldesk`, `frosty-shape-12386475`, `aws-eu-central-1`).
- **Connection:** the Worker reaches Neon through **Cloudflare Hyperdrive**, which keeps a warm connection pool near the database.
- **Address:** desks connect to `https://hosteldesk-sync.ableabenaitwe.workers.dev`, the `DEFAULT_SERVER_URL` in `crates/app/src/sync.rs`.

The Neon connection string lives only in `~/.hosteldesk/neon_url` (mode 0600), next to the license signing key. Never commit it.

## One-time setup

1. **Workers Paid plan** ($5/month). Sign-in hashes passwords with Argon2, which needs more CPU than the Free plan's 10 ms per request allows. Hyperdrive is included in the plan.
2. **Log wrangler in.** Wrangler prefers a `CLOUDFLARE_API_TOKEN` environment variable over its own login. If your shell sets one for another project, unset it for wrangler commands:

   ```sh
   env -u CLOUDFLARE_API_TOKEN npx -y wrangler@4 login
   ```

3. **Create the Hyperdrive config.** It points at Neon's direct (unpooled) endpoint. Put the printed `id` into `crates/server/wrangler.toml` under `[[hyperdrive]]`:

   ```sh
   cd crates/server
   env -u CLOUDFLARE_API_TOKEN npx -y wrangler@4 hyperdrive create hosteldesk \
     --connection-string="$(sed 's/?.*//' ~/.hosteldesk/neon_url)"
   ```

4. **Signing key secret (required).** Licenses live on accounts, and the Worker signs each desk's lease with the production license key. Without this secret no desk can unlock. Pipe it in so it never shows on screen or in shell history:

   ```sh
   env -u CLOUDFLARE_API_TOKEN npx -y wrangler@4 secret put HOSTELDESK_SIGNING_KEY < ~/.hosteldesk/signing_key.b64
   ```

## Every deploy

```sh
env -u CLOUDFLARE_API_TOKEN scripts/deploy-worker.sh   # migrates Neon, then wrangler deploy
scripts/smoke-worker.sh https://hosteldesk-sync.ableabenaitwe.workers.dev
```

The smoke test creates two throwaway organizations named "Smoke" and "Other". Delete them afterwards if you want a clean database.

**Logs:** run `npx -y wrangler@4 tail hosteldesk-sync`, or open Workers → hosteldesk-sync → Observability in the Cloudflare dashboard.

## Operations

Licensing has its own script. Run everything else with `psql "$(cat ~/.hosteldesk/neon_url)" -c "…"`:

| Task | SQL |
|---|---|
| Sell, renew, show or end a license | `scripts/license-account.sh <email> 1-year` (see `LICENSING.md`) |
| Find an organization by email | `SELECT o.id, o.name, o.license_plan, o.license_expires_on FROM organizations o JOIN accounts a ON a.org_id = o.id WHERE a.email = '<email>'` |
| Revoke a lost or stolen computer | `UPDATE devices SET revoked_at = now() WHERE device_id = '<id>'` |
| List an organization's computers | `SELECT device_id, name, device_prefix, created_at, revoked_at FROM devices WHERE org_id = '<org id>'` |

Neon keeps restorable history for a short window (hours) on the Free plan, and for longer on paid plans. Raise it in the Neon console (Project settings → Storage) before you have real customers, or move the project to a paid org.

## Schema changes

- Add a new numbered file in `crates/server/migrations/` (for example `0002_add_x.sql`). Never edit one that has been applied.
- `scripts/migrate-sync-db.sh` applies pending files in order, each in its own transaction, and records them in `schema_migrations`. `scripts/deploy-worker.sh` runs it before the new Worker goes live.
- Keep each change backward compatible with the Worker that is still running.
