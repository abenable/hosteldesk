# Licensing

HostelDesk is proprietary, closed-source software — see [`LICENSE`](../LICENSE) for the source code's terms and [`EULA.md`](../EULA.md) for the end-user terms governing the compiled application. Contact ableabenaitwe@gmail.com for licensing, purchasing, or support questions.

The app itself is license-gated. On startup it verifies an Ed25519-signed license and shows an activation screen when it is missing, invalid, or expired. Verification is fully offline. Activation is file-based only: the app opens a native file picker and reads the selected `.hdlicense`/`.lic`/`.txt` file — there is no paste-a-key text field.

The public key is baked into `crates/license/src/lib.rs`; the matching production signing key lives at `~/.hosteldesk/signing_key.b64` (outside the repository, `chmod 600`) — back it up somewhere durable (a password manager), since losing it means no more licenses can ever be issued against the public key already baked into shipped builds.

## Issuing a license

`crates/license/examples/issue_license.rs` mints a license file for one of four fixed plans, each just a duration off the activation date (a semester = 6 months):

| Plan | Duration |
|---|---|
| `trial` | 1 semester (6 months) |
| `1-year` | 2 semesters (12 months) |
| `3-year` | 36 months |
| `5-year` | 60 months |

```sh
cargo run --example issue_license -p license -- \
  --hostel-name "Ocean View Hostel" \
  --plan 1-year \
  --key-file ~/.hosteldesk/signing_key.b64
```

This writes a `<slug>-<plan>.hdlicense` file — send that file to the customer; they pick it on the app's activation screen.

## Development licenses

Builds with the `dev-license` Cargo feature additionally accept a development-only key, so contributors can generate local licenses without the production key:

```sh
cargo run --example generate_dev_license -p license > dev.hdlicense
cargo tauri dev --features dev-license
```

(or `cargo tauri build --debug --features dev-license` for a debug bundle), then pick `dev.hdlicense` on the activation screen. The feature is off by default — including for a plain `cargo tauri build --debug` — so a shippable build can never accept a dev-minted license.
