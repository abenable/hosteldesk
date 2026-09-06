//! Mints a real, production-signed license file for a paying customer.
//!
//! Usage:
//!   cargo run --example issue_license -p license -- \
//!     --hostel-name "Sunny Hostel" \
//!     --plan 1-year \
//!     --key-file ~/.hosteldesk/signing_key.b64 \
//!     [--out sunny-hostel-1-year.hdlicense]
//!
//! `--key-file` must hold the base64-encoded production Ed25519 private key
//! that matches `PUBLIC_KEY_BASE64` in src/lib.rs. That key is never
//! committed to this repository — keep it somewhere only you control.

use base64::{engine::general_purpose::STANDARD, Engine};
use chrono::{Months, Utc};
use ed25519_dalek::{Signer, SigningKey};
use license::LicensePayload;
use std::path::PathBuf;

struct Plan {
    tier: &'static str,
    months: u32,
}

fn plan_for(name: &str) -> Option<Plan> {
    match name {
        "trial" => Some(Plan {
            tier: "trial",
            months: 6,
        }),
        "1-year" => Some(Plan {
            tier: "1-year",
            months: 12,
        }),
        "3-year" => Some(Plan {
            tier: "3-year",
            months: 36,
        }),
        "5-year" => Some(Plan {
            tier: "5-year",
            months: 60,
        }),
        _ => None,
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut hostel_name: Option<String> = None;
    let mut plan_name: Option<String> = None;
    let mut key_file: Option<PathBuf> = None;
    let mut out: Option<PathBuf> = None;

    let mut args = std::env::args().skip(1);
    while let Some(flag) = args.next() {
        let mut value = || -> Result<String, String> {
            args.next().ok_or_else(|| format!("{flag} needs a value"))
        };
        match flag.as_str() {
            "--hostel-name" => hostel_name = Some(value()?),
            "--plan" => plan_name = Some(value()?),
            "--key-file" => key_file = Some(PathBuf::from(value()?)),
            "--out" => out = Some(PathBuf::from(value()?)),
            other => return Err(format!("unknown flag: {other}").into()),
        }
    }

    let hostel_name = hostel_name.ok_or("--hostel-name is required")?;
    let plan_name = plan_name.ok_or("--plan is required (trial | 1-year | 3-year | 5-year)")?;
    let key_file = key_file.ok_or("--key-file is required")?;
    let plan = plan_for(&plan_name).ok_or_else(|| {
        format!("unknown plan '{plan_name}', expected trial | 1-year | 3-year | 5-year")
    })?;

    let key_b64 = std::fs::read_to_string(&key_file)
        .map_err(|e| format!("could not read key file {}: {e}", key_file.display()))?;
    let key_bytes: [u8; 32] = STANDARD
        .decode(key_b64.trim())?
        .try_into()
        .map_err(|v: Vec<u8>| format!("signing key must be 32 bytes, got {}", v.len()))?;
    let signing_key = SigningKey::from_bytes(&key_bytes);

    let expiry_date = Utc::now()
        .date_naive()
        .checked_add_months(Months::new(plan.months))
        .ok_or("date overflow computing license expiry")?;

    let payload = LicensePayload {
        hostel_name: hostel_name.clone(),
        expiry_date,
        tier: plan.tier.to_string(),
    };
    let json = serde_json::to_vec(&payload)?;
    let signature = signing_key.sign(&json);
    let license = format!(
        "{}:{}",
        STANDARD.encode(signature.to_bytes()),
        STANDARD.encode(&json)
    );

    let out = out.unwrap_or_else(|| {
        let slug: String = hostel_name
            .to_lowercase()
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
            .collect();
        PathBuf::from(format!("{slug}-{}.hdlicense", plan.tier))
    });
    std::fs::write(&out, &license)?;

    println!(
        "Issued {} license for '{}', expires {}",
        plan.tier, hostel_name, expiry_date
    );
    println!("Wrote {}", out.display());
    Ok(())
}
