fn main() {
    // sqlx embeds migration files via include_str!, but Cargo only learns about
    // those file dependencies after the macro has run. Emitting the dependency here
    // ensures the crate is rebuilt whenever a migration is added or changed,
    // preventing stale embedded migrations during local development.
    println!("cargo:rerun-if-changed=migrations");
}
