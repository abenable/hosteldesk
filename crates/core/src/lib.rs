//! Pure domain logic for Hostel Desk.
//!
//! This crate contains the hostel business model and validation rules. It has no
//! dependencies on Tauri, SQL, or any frontend technology, so it can be unit
//! tested in isolation and reused by future tools (CLI, web API, etc.).

pub mod currency;
pub use currency::*;
