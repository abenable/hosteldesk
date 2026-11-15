# Deploying the sync Worker

The sync API (`crates/server`) is a Rust Cloudflare Worker named `hosteldesk-sync`. It stores data in a D1 database named `hosteldesk`. Desks reach it at `https://hosteldesk-sync.<your-subdomain>.workers.dev`, the default in `crates/app/src/sync.rs` (`DEFAULT_SERVER_URL`).

## One-time setup

1. **Workers Paid plan** ($5/month). Sign-in hashes passwords with Argon2, which needs more CPU than the Free plan's 10 ms per request allows.
2. **Log in**: `npx -y wrangler@4 login`.
3. **Create the database** and put its id into `crates/server/wrangler.toml` (`database_id`):

   ```sh
   cd crates/server
   npx -y wrangler@4 d1 create hosteldesk
   ```

4. **Signing key secret.** Leases are signed with the production license key. Pipe it in so it never shows on screen or in shell history:

   ```sh
   npx -y wrangler@4 secret put HOSTELDESK_SIGNING_KEY < ~/.hosteldesk/signing_key.b64
   ```

## Every deploy

```sh
scripts/deploy-worker.sh          # D1 migrations, then wrangler deploy
scripts/smoke-worker.sh https://hosteldesk-sync.<your-subdomain>.workers.dev
```

The smoke test creates two throwaway organizations named "Smoke" and "Other". Delete them afterwards if you want a clean database. The test's sign-in lockout check also locks its own throwaway email for 15 minutes.

Logs: `npx -y wrangler@4 tail hosteldesk-sync`, or Workers → hosteldesk-sync → Observability in the dashboard.

## Operations

All of these run from `crates/server` with `npx -y wrangler@4 d1 execute hosteldesk --remote --command "…"`:

| Task | SQL |
|---|---|
| License a paying organization | `UPDATE organizations SET licensed = 1 WHERE id = '<org id>'` |
| Find an organization by email | `SELECT o.id, o.name, o.licensed FROM organizations o JOIN accounts a ON a.org_id = o.id WHERE a.email = '<email>'` |
| Revoke a lost or stolen computer | `UPDATE devices SET revoked_at = datetime('now') WHERE device_id = '<id>'` |
| List an organization's computers | `SELECT device_id, name, device_prefix, created_at, revoked_at FROM devices WHERE org_id = '<org id>'` |

D1 keeps 30 days of point-in-time history (`wrangler d1 time-travel`), which serves as the server-side backup.

## Schema changes

Add a new numbered file in `crates/server/migrations/`; never edit one that has been applied. `scripts/deploy-worker.sh` applies it before the new Worker goes live. Keep each change backward compatible with the Worker version that is still running, because migrations run first.
