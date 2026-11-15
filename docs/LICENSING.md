# Licensing

HostelDesk is proprietary, closed-source software — see [`LICENSE`](../LICENSE) for the source code's terms and [`EULA.md`](../EULA.md) for the end-user terms governing the compiled application. Contact ableabenaitwe@gmail.com for licensing, purchasing, or support questions.

## Licenses live on the account, and the server decides

A HostelDesk license belongs to a **HostelDesk Cloud account** (an organization), not to a file or a computer. The cloud backend is the only authority. There are no license files and no signing keys.

- **Plan and end date.** Each account has a plan (`trial`, `semester`, `1-year`, `3-year`, `5-year`) and the day it ends, stored in the sync database (`organizations.license_plan`, `organizations.license_expires_on`).
- **You approve every account.** A new account is **pending** until you approve it (`organizations.approved_at`). While pending, its computers stay locked with "waiting for approval", and the server refuses to sync its data (HTTP 403).
- **Activation is signing in.** On a new computer the lock screen asks the user to sign in or create an account. After signing in, the desk asks the server `POST /v1/license` (authenticated by its device token, over HTTPS) and unlocks once the account is approved and while its plan runs.
- **Re-checks.** The desk asks again on every sync, every 30 seconds while online. A plan you change or end takes effect on the desk's next sync.
- **Offline.** The desk caches the server's last answer (`account_license.json` in its data directory) and keeps working offline for up to **14 days** after its last successful check (`OFFLINE_GRACE_DAYS`). Past that, or if the clock is set back before the last check, it locks until it reaches the server again. The lock screen retries on its own.
- **Free trial.** New accounts start on a **30-day free trial** (`TRIAL_DAYS` in `crates/server/src/lib.rs`). The trial is stamped on the account when it is created, and its clock runs from that moment, including while the account waits for approval.

The cache is not signed: a user who edits it can stretch the offline window. The account's plan itself lives only on the server and cannot be changed from a desk.

## Approving, selling, renewing and ending

```sh
scripts/license-account.sh pending                     # accounts waiting for approval
scripts/license-account.sh owner@hostel.com approve    # let the account in
scripts/license-account.sh owner@hostel.com show       # approval, plan, end date
scripts/license-account.sh owner@hostel.com 1-year     # sell a plan
scripts/license-account.sh owner@hostel.com end        # end the license today
```

Approved computers unlock on their own within about 30 seconds.

A sold plan runs from today. If the account's current end date is still ahead, it runs from that date instead, so renewing early never loses days.

## Upgrading from v1.x

v1.x license files are no longer read. After upgrading, a computer shows the sign-in screen.
- **The customer:** creates an account (or signs in). Once you approve it, the computer unlocks on the 30-day trial and uploads its existing hostel data.
- **You:** if they had already paid, sell their account the matching plan with `scripts/license-account.sh`.

## Development

There is no developer shortcut: a desk unlocks only through a server. For local work, run the Worker locally (`docs/DEVELOPMENT.md`). In a debug build, enter `http://127.0.0.1:8787` as the server under **Advanced** on the sign-in screen, then create an account and approve it with `SYNC_DATABASE_URL=<local url> scripts/license-account.sh <email> approve`.
