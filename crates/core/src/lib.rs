//! Currency parsing shared between the command layer and the database layer.
//!
//! This crate is currently just [`currency`]: parsing and validating monetary
//! amount strings. It has no dependency on Tauri or SQL, so both `crates/app`
//! and `crates/db` can depend on it without a cycle.

pub mod currency;
pub use currency::*;
