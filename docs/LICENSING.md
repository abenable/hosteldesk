# Licensing

HostelDesk is proprietary, closed-source software — see [`LICENSE`](../LICENSE) for the source code's terms and [`EULA.md`](../EULA.md) for the end-user terms governing the compiled application. Contact ableabenaitwe@gmail.com for licensing, purchasing, or support questions.

## Licenses live on the account

A HostelDesk license belongs to a **HostelDesk Cloud account** (an organization), not to a file or a computer. The backend decides who is licensed and until when. The app only checks what the backend signs.

- **Plan and end date.** Each account has a plan (`trial`, `semester`, `1-year`, `3-year`, `5-year`) and the date it ends, stored in the sync database (`organizations.license_plan`, `organizations.license_expires_on`).
- **Activation is signing in.** On a new computer the lock screen asks the user to sign in or create an account. While the account's plan runs, the server returns a **lease**: an Ed25519-signed license in the same format as the old license files.
- **Leases.** The app saves the lease beside its database and verifies it offline against the public key in `crates/license/src/lib.rs`. A lease lasts 14 days, and never past the account's end date. Every sync renews it.
- **When the plan ends:** a computer that stops syncing, or whose account ended, locks within 14 days. When it can reach the server again, the lock screen renews the lease by itself.
- **Free trial.** New accounts start on a **30-day free trial** (`TRIAL_DAYS` in `crates/server/src/lib.rs`).
- **Signing key.** Leases are signed with the production signing key, set as the Worker secret `HOSTELDESK_SIGNING_KEY` (see `DEPLOY.md`). Without it the server issues no leases and no new computer can unlock.

The production signing key lives at `~/.hosteldesk/signing_key.b64` (outside the repository, `chmod 600`). Back it up somewhere durable, such as a password manager. Losing it means no lease can ever verify against the public key already baked into shipped builds.

## Selling, renewing and ending a license

```sh
scripts/license-account.sh owner@hostel.com show       # plan and end date
scripts/license-account.sh owner@hostel.com 1-year     # sell a plan
scripts/license-account.sh owner@hostel.com end        # end the license today
```

A sold plan runs from today. If the account's current end date is still ahead, it runs from that date instead, so renewing early never loses days. Desks pick up the change on their next sync.

## Upgrading from v1.x license files

Activating a license file is gone from the app. A computer upgraded from v1.x keeps working on its existing license file until that file expires. To keep using HostelDesk after that, the user signs in to an account; their hostel data uploads to it. For a customer who had already paid, sell their account the matching plan with `scripts/license-account.sh`.

## Development

Builds with the `dev-license` Cargo feature also accept a development-only key that is committed to the repo. A shippable build never does, because the feature is off by default. For local work, either:

- put a dev license where the app keeps its lease:

  ```sh
  cargo run -q -p license --example generate_dev_license > ~/.local/share/com.hosteldesk.app/license.lic
  cargo run -p app --features dev-license
  ```

- or run the Worker locally with the dev key (`HOSTELDESK_SIGNING_KEY=KLXBx9s0JhWFbQhtZX2IO34NajBXIy5Kor55yzOIp0E=` in `crates/server/.dev.vars`), then sign in from a `dev-license` build with the server set to `http://127.0.0.1:8787` (under Advanced).
