//! Signs a license payload with the dev-only key so `cargo run --example
//! generate_dev_license` can unblock local development without the
//! production signing key. Only accepted by debug builds (see
//! `DEV_PUBLIC_KEY_BASE64` in src/lib.rs) — never valid in a release build.

use base64::{engine::general_purpose::STANDARD, Engine};
use chrono::{Days, Utc};
use ed25519_dalek::{Signer, SigningKey};
use license::LicensePayload;

/// Matches the private half of `DEV_PUBLIC_KEY_BASE64` in src/lib.rs.
const DEV_PRIVATE_KEY_BASE64: &str = "KLXBx9s0JhWFbQhtZX2IO34NajBXIy5Kor55yzOIp0E=";

fn main() {
    let key_bytes: [u8; 32] = STANDARD
        .decode(DEV_PRIVATE_KEY_BASE64)
        .expect("valid base64")
        .try_into()
        .expect("32 bytes");
    let signing_key = SigningKey::from_bytes(&key_bytes);

    let payload = LicensePayload {
        hostel_name: "Dev Hostel".to_string(),
        expiry_date: Utc::now()
            .date_naive()
            .checked_add_days(Days::new(3650))
            .expect("valid date"),
        tier: "pro".to_string(),
    };
    let json = serde_json::to_vec(&payload).expect("payload serializes");
    let signature = signing_key.sign(&json);

    println!(
        "{}:{}",
        STANDARD.encode(signature.to_bytes()),
        STANDARD.encode(&json)
    );
}
