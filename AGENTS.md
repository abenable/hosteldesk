# GLOBAL SYSTEM INSTRUCTIONS FOR AGENT

## 1. Architecture

This is a **Tauri v2** project with a **Rust backend** and a **React / TypeScript / Tailwind** frontend, organized as a **Cargo workspace** with the following members:

- `core` — currency parsing shared between `app` and `db` (see `hosteldesk-core`)
- `db` — persistence layer (SQLite via `sqlx`)
- `pdf` — PDF generation
- `license` — licensing logic
- `app` — the Tauri application shell that wires everything together

## 2. Tech Stack

- **Rust**
- **Tauri v2**
- **sqlx** (SQLite)
- **React**
- **TypeScript**
- **TailwindCSS**
- **rust_decimal`
- **chrono**

## 3. Currency Constraint

**NEVER** use `f64` or `f32` for currency. **Always** use `rust_decimal` (`rust_decimal::Decimal`) for all monetary values, calculations, and storage. Floating-point types are forbidden anywhere a currency value is represented.

## 4. Workflow

Follow **Test-Driven Development (TDD)**:

1. Write a failing test in Rust that describes the desired behavior.
2. Run the test and confirm it fails for the expected reason.
3. Implement the minimum logic required to make the test pass.
4. Refactor with confidence, keeping tests green.

## 5. Verification

After **every significant change**, run the following and do **not** proceed if any of them produce warnings or errors:

```sh
cargo check
cargo clippy
cargo test
npm run lint
```

Treat warnings as failures. Fix the root cause; do not suppress or ignore them.

## 6. Documentation

Document **all public Rust structs, enums, and functions** with doc comments (`///`). Documentation should describe purpose and behavior; non-obvious parameters and return values should be explained.

## 7. Self-Review

Before completing any task, review the code against these security and quality practices:

- **SQL injection prevention** — use `sqlx::query`/`sqlx::query_as::<_, T>` with bound (`?N`) parameters, never string-interpolated SQL. Prefer the runtime `query_as::<_, T>` form over the `query!`/`query_as!` macros: the macros need a live `DATABASE_URL` (or a committed `.sqlx/` cache) to compile, which this project deliberately avoids so CI never needs a database just to build.
- **Error handling** — use `Result<T, E>` and propagate errors explicitly. Avoid panics in normal control flow.
- **No `.unwrap()` in production code** — use `?`, `.context(...)`, `unwrap_or`, `unwrap_or_else`, or explicit `match` instead. `.unwrap()` / `.expect()` are acceptable only in tests.

## 8. Migrations

Migrations live in `crates/db/migrations/` and are applied by `sqlx::migrate!`.

- **`20260905000000_initial_schema.sql` is frozen as of the `v1.0.0` tag.** It was re-baselined into a single file immediately before release, which is only safe because nothing had shipped yet. Once `v1.0.0` is tagged, that precondition no longer holds — never edit this file again; fix forward with a new timestamped migration, per the append-only rule below.
- Migrations are **append-only** once shipped or applied to any teammate's or user's database. Never edit a migration file that has ever run against a real DB; fix forward with a new timestamped file.
- A broken development DB can be recovered by deleting `<app-data-dir>/hosteldesk.db*` (including `-wal` and `-shm` files) and relaunching the app.
- Table-rewrite migrations must use explicit column lists: `INSERT INTO new (a, b) SELECT a, b FROM old`. Never use `SELECT *`, because column-order drift silently corrupts data.
- A migration that drops indexes must re-create the equivalent indexes at the end of the same file.
- Use `-- no-transaction` only for migrations that toggle `PRAGMA foreign_keys`; everything else runs inside sqlx's default transaction.

## 9. Branching and Releases

- Every release — **major or minor** — is developed on a **version branch** named after the upcoming release (e.g. `v1.0.1`, `v2.0.0`). **All changes for that release go on the version branch** — not directly on `main`.
- To cut a release: bump `version` in `crates/app/tauri.conf.json` (and `frontend/package.json` to match), merge the version branch into `main`, then tag:

  ```sh
  git tag v1.0.1
  git push origin main v1.0.1
  ```

- The tag push triggers `.github/workflows/release.yml`, which builds the Ubuntu `.deb` (22.04) and Windows NSIS `.exe` + MSI, uploads platform checksum files, and publishes the GitHub release. CI (`.github/workflows/ci.yml`) runs clippy `-D warnings`, tests, lint, and the frontend build on every `main` push and PR.
- Do not move or re-tag a version tag once its release run has started; if a release fails, fix forward on `main` and cut the next version tag.
