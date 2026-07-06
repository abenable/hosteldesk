use base64::{engine::general_purpose::STANDARD, Engine};
use chrono::NaiveDate;
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// A verified license payload describes the licensed hostel, expiration date, and tier.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LicensePayload {
    /// Name of the licensed hostel.
    pub hostel_name: String,
    /// License expiration date.
    pub expiry_date: NaiveDate,
    /// Licensed feature tier.
    pub tier: String,
}

/// License generator helpers. Only compiled when the `generator` feature is enabled.
#[cfg(feature = "generator")]
pub mod generator;

/// Errors that can occur during license verification.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum LicenseError {
    /// The license string did not contain the expected `signature:payload` separator.
    #[error("invalid license format, expected 'signature:payload'")]
    InvalidFormat,
    /// The base64-encoded signature could not be decoded.
    #[error("unable to decode signature")]
    InvalidSignatureEncoding,
    /// The base64-encoded payload could not be decoded.
    #[error("unable to decode license payload")]
    InvalidPayloadEncoding,
    /// The embedded public key could not be parsed.
    #[error("unable to load verification key")]
    InvalidPublicKey,
    /// The signature does not match the payload.
    #[error("license signature is invalid")]
    BadSignature,
    /// The signature is valid but the license has expired.
    #[error("license expired on {0}")]
    Expired(NaiveDate),
    /// The payload JSON could not be parsed.
    #[error("unable to parse license payload")]
    InvalidPayload,
}

/// Base64-encoded Ed25519 public key baked into the application.
///
/// The matching private key lives outside this repository and is used to sign
/// customer license payloads.
pub const PUBLIC_KEY_BASE64: &str = "7hhviziBM6uuXQ3aoqAE+E0sqANqvJbHWZSWk87DYas=";

/// Separator between the base64 signature and base64 payload.
const SEPARATOR: char = ':';

/// Verify a base64-encoded signed license string against the baked-in public key
/// and the current system date.
///
/// The expected format is `base64(signature):base64(json_payload)`.
pub fn verify_license(license: &str) -> Result<LicensePayload, LicenseError> {
    verify_license_at(license, chrono::Utc::now().date_naive())
}

/// Verify a license string as of a specific date.
///
/// Useful for tests and for consumers that want to control the current date.
pub fn verify_license_at(license: &str, today: NaiveDate) -> Result<LicensePayload, LicenseError> {
    let (signature, payload) = split_license(license)?;
    let verifying_key = load_public_key()?;
    verifying_key
        .verify(&payload, &signature)
        .map_err(|_| LicenseError::BadSignature)?;

    let license: LicensePayload =
        serde_json::from_slice(&payload).map_err(|_| LicenseError::InvalidPayload)?;

    if license.expiry_date < today {
        return Err(LicenseError::Expired(license.expiry_date));
    }

    Ok(license)
}

/// Split a license string into its decoded signature and payload bytes.
fn split_license(license: &str) -> Result<(Signature, Vec<u8>), LicenseError> {
    let (signature_b64, payload_b64) = license
        .split_once(SEPARATOR)
        .ok_or(LicenseError::InvalidFormat)?;

    let signature_bytes: [u8; Signature::BYTE_SIZE] = STANDARD
        .decode(signature_b64)
        .map_err(|_| LicenseError::InvalidSignatureEncoding)?
        .try_into()
        .map_err(|_| LicenseError::InvalidSignatureEncoding)?;
    let signature = Signature::from_bytes(&signature_bytes);

    let payload = STANDARD
        .decode(payload_b64)
        .map_err(|_| LicenseError::InvalidPayloadEncoding)?;

    Ok((signature, payload))
}

/// Load the baked-in Ed25519 public key.
fn load_public_key() -> Result<VerifyingKey, LicenseError> {
    let bytes: [u8; 32] = STANDARD
        .decode(PUBLIC_KEY_BASE64)
        .map_err(|_| LicenseError::InvalidPublicKey)?
        .try_into()
        .map_err(|_| LicenseError::InvalidPublicKey)?;

    VerifyingKey::from_bytes(&bytes).map_err(|_| LicenseError::InvalidPublicKey)
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::{engine::general_purpose::STANDARD, Engine};
    use ed25519_dalek::{Signer, SigningKey};

    /// Deterministic signing key used only in tests.
    ///
    /// The public key must match [`PUBLIC_KEY_BASE64`].
    fn signing_key() -> SigningKey {
        // Decoded seed bytes from the key generation step that produced
        // PUBLIC_KEY_BASE64 = "7hhviziBM6uuXQ3aoqAE+E0sqANqvJbHWZSWk87DYas="
        let seed_b64 = "R5IjGWlJwwSgPqamES9RPlZ2Ts2RTHvnRWV9VHzUp3g=";
        let seed = STANDARD
            .decode(seed_b64)
            .expect("seed bytes are valid base64");
        let seed: [u8; 32] = seed.try_into().expect("seed is 32 bytes");
        SigningKey::from_bytes(&seed)
    }

    #[test]
    fn test_public_key_matches_signing_key() {
        let key = signing_key();
        let public_b64 = STANDARD.encode(key.verifying_key().as_bytes());
        assert_eq!(public_b64, PUBLIC_KEY_BASE64);
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

    #[test]
    fn accepts_valid_unexpired_license() {
        let today = NaiveDate::from_ymd_opt(2025, 1, 1).unwrap();
        let payload = LicensePayload {
            hostel_name: "Sunny Hostel".to_string(),
            expiry_date: NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(),
            tier: "pro".to_string(),
        };
        let license = sign_payload(&payload);

        let result = verify_license_at(&license, today).unwrap();

        assert_eq!(result, payload);
    }

    #[test]
    fn rejects_tampered_license() {
        let today = NaiveDate::from_ymd_opt(2025, 1, 1).unwrap();
        let payload = LicensePayload {
            hostel_name: "Sunny Hostel".to_string(),
            expiry_date: NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(),
            tier: "pro".to_string(),
        };
        let json = serde_json::to_vec(&payload).expect("payload serializes");
        let signature = signing_key().sign(&json);

        // Tamper with the payload without updating the signature.
        let tampered = LicensePayload {
            hostel_name: "Evil Hostel".to_string(),
            ..payload
        };
        let tampered_json = serde_json::to_vec(&tampered).expect("payload serializes");

        let license = format!(
            "{}:{}",
            STANDARD.encode(signature.to_bytes()),
            STANDARD.encode(tampered_json)
        );

        assert!(matches!(
            verify_license_at(&license, today),
            Err(LicenseError::BadSignature)
        ));
    }

    #[test]
    fn rejects_expired_license() {
        let today = NaiveDate::from_ymd_opt(2025, 6, 1).unwrap();
        let payload = LicensePayload {
            hostel_name: "Sunny Hostel".to_string(),
            expiry_date: NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
            tier: "pro".to_string(),
        };
        let license = sign_payload(&payload);

        assert!(matches!(
            verify_license_at(&license, today),
            Err(LicenseError::Expired(_))
        ));
    }

    #[test]
    fn rejects_malformed_license_string() {
        assert!(matches!(
            verify_license_at("no-separator", NaiveDate::MIN),
            Err(LicenseError::InvalidFormat)
        ));
    }
}
