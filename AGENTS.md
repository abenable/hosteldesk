# GLOBAL SYSTEM INSTRUCTIONS FOR AGENT

## 1. Architecture

This is a **Tauri v2** project with a **Rust backend** and a **React / TypeScript / Tailwind** frontend, organized as a **Cargo workspace** with the following members:

- `core` — domain logic and shared business rules
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

- **SQL injection prevention** — use `sqlx` query macros (`query!` / `query_as!`) with bound parameters. Never interpolate user input into SQL strings.
- **Error handling** — use `Result<T, E>` and propagate errors explicitly. Avoid panics in normal control flow.
- **No `.unwrap()` in production code** — use `?`, `.context(...)`, `unwrap_or`, `unwrap_or_else`, or explicit `match` instead. `.unwrap()` / `.expect()` are acceptable only in tests.

## 8. Migrations

Migrations live in `crates/db/migrations/` and are applied by `sqlx::migrate!`.

- Migrations are **append-only** once shipped or applied to any teammate's or user's database. Never edit a migration file that has ever run against a real DB; fix forward with a new timestamped file.
- A broken development DB can be recovered by deleting `<app-data-dir>/hosteldesk.db*` (including `-wal` and `-shm` files) and relaunching the app.
- Table-rewrite migrations must use explicit column lists: `INSERT INTO new (a, b) SELECT a, b FROM old`. Never use `SELECT *`, because column-order drift silently corrupts data.
- A migration that drops indexes must re-create the equivalent indexes at the end of the same file.
- Use `-- no-transaction` only for migrations that toggle `PRAGMA foreign_keys`; everything else runs inside sqlx's default transaction.
