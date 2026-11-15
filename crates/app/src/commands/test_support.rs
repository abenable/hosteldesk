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
    let today = chrono::Utc::now().date_naive();
    state.set(crate::state::AccountLicense {
        account_name: "Money Test Hostel".to_string(),
        plan: "trial".to_string(),
        expires_on: today + chrono::Duration::days(30),
        checked_on: today,
    });
    state
}
