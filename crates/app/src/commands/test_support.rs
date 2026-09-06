use crate::LicenseState;
use std::sync::atomic::{AtomicU64, Ordering};

pub(crate) static COUNTER: AtomicU64 = AtomicU64::new(0);

pub(crate) fn temp_db_url() -> String {
    let index = COUNTER.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "hosteldesk_app_test_{}_{}.db",
        std::process::id(),
        index
    ));
    format!("sqlite:{}", path.display())
}

pub(crate) fn setup_license_state() -> LicenseState {
    let state = LicenseState::default();
    state.set(license::LicensePayload {
        hostel_name: "Money Test Hostel".to_string(),
        expiry_date: chrono::Utc::now().date_naive() + chrono::Duration::days(30),
        tier: "trial".to_string(),
    });
    state
}
