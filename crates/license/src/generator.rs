use crate::LicensePayload;
use base64::{engine::general_purpose::STANDARD, Engine};
use chrono::Duration;
use ed25519_dalek::{Signer, SigningKey};

/// Deterministic test signing key.
///
/// This is the private counterpart to [`crate::PUBLIC_KEY_BASE64`]. It is only
/// compiled in when the `generator` feature is enabled and should not be used to
/// sign production customer licenses.
fn signing_key() -> SigningKey {
    let seed_b64 = "R5IjGWlJwwSgPqamES9RPlZ2Ts2RTHvnRWV9VHzUp3g=";
    let seed = STANDARD
        .decode(seed_b64)
        .expect("seed bytes are valid base64");
    let seed: [u8; 32] = seed.try_into().expect("seed is 32 bytes");
    SigningKey::from_bytes(&seed)
}

/// Sign a license payload and return the `base64(signature):base64(json)` string.
fn sign_payload(payload: &LicensePayload) -> String {
    let json = serde_json::to_vec(payload).expect("payload serializes");
    let signature = signing_key().sign(&json);
    format!(
        "{}:{}",
        STANDARD.encode(signature.to_bytes()),
        STANDARD.encode(&json)
    )
}

/// Generate a 30-day trial license for the given hostel name.
///
/// The returned string can be passed to [`crate::verify_license`] and is valid
/// until 30 days from the current UTC date.
pub fn generate_trial_license(hostel_name: &str) -> String {
    let expiry = chrono::Utc::now().date_naive() + Duration::days(30);
    let payload = LicensePayload {
        hostel_name: hostel_name.to_string(),
        expiry_date: expiry,
        tier: "trial".to_string(),
    };
    sign_payload(&payload)
}
