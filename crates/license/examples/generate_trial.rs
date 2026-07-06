use base64::{engine::general_purpose::STANDARD, Engine};
use ed25519_dalek::{Signer, SigningKey};
use license::LicensePayload;

/// Deterministic test signing key used only in tests/examples.
///
/// The generated trial license is valid only for builds that embed the matching
/// public key (`license::PUBLIC_KEY_BASE64`). Do not use this key for production
/// customer licenses.
fn signing_key() -> SigningKey {
    let seed_b64 = "R5IjGWlJwwSgPqamES9RPlZ2Ts2RTHvnRWV9VHzUp3g=";
    let seed = STANDARD
        .decode(seed_b64)
        .expect("seed bytes are valid base64");
    let seed: [u8; 32] = seed.try_into().expect("seed is 32 bytes");
    SigningKey::from_bytes(&seed)
}

fn sign_payload(payload: &LicensePayload) -> String {
    let json = serde_json::to_vec(payload).expect("payload serializes");
    let signature = signing_key().sign(&json);
    format!(
        "{}:{}",
        STANDARD.encode(signature.to_bytes()),
        STANDARD.encode(&json)
    )
}

fn main() {
    let today = chrono::Utc::now().date_naive();
    let expiry = today
        .checked_add_signed(chrono::Duration::days(30))
        .expect("valid date");

    let payload = LicensePayload {
        hostel_name: "Trial Hostel".to_string(),
        expiry_date: expiry,
        tier: "trial".to_string(),
    };

    let license = sign_payload(&payload);
    println!("Expiry: {expiry}");
    println!("License:");
    println!("{license}");
}
