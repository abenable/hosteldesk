use std::sync::Mutex;
use std::time::{Duration, Instant};

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use tracing::error;

/// Days a computer keeps working without reaching the server. Past that it
/// locks until it can ask the server again.
pub const OFFLINE_GRACE_DAYS: i64 = 14;

/// What the server last said about this computer's account license.
///
/// The server is the only authority; this is a cache so the desk keeps
/// working offline for [`OFFLINE_GRACE_DAYS`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccountLicense {
    /// Organization name.
    pub account_name: String,
    /// `trial`, `semester`, `1-year`, `3-year`, or `5-year`.
    pub plan: String,
    /// Last day the account's license is valid.
    pub expires_on: NaiveDate,
    /// Day this computer last heard it from the server.
    pub checked_on: NaiveDate,
    /// The owner has approved the account. A pending account is never active.
    #[serde(default)]
    pub approved: bool,
}

impl AccountLicense {
    /// Usable on `today`: the account is approved, the plan has not ended,
    /// the server confirmed it within [`OFFLINE_GRACE_DAYS`], and the clock
    /// has not been wound back before that confirmation.
    pub fn is_active(&self, today: NaiveDate) -> bool {
        self.approved
            && today <= self.expires_on
            && self.checked_on <= today
            && (today - self.checked_on).num_days() <= OFFLINE_GRACE_DAYS
    }
}

/// A session with no activity for this long is treated as signed out. Every
/// guarded command reads the session via [`AuthState::current_session`], so
/// this is the single choke point that enforces it.
const SESSION_IDLE_TIMEOUT: Duration = Duration::from_secs(30 * 60);

/// In-memory authorization session for the application's single window.
///
/// This is the backend source of truth for who is currently signed in. The
/// frontend may cache the returned user for display, but it must never be
/// trusted for an access-control decision.
#[derive(Debug, Clone)]
pub struct Session {
    /// Authenticated staff user id.
    pub user_id: i64,
    /// Hostel whose data the user may access.
    pub hostel_id: i64,
    /// Role assigned to the user (`admin` or `manager`).
    pub role: String,
}

/// Current session storage for the single-window desktop app.
#[derive(Debug, Default)]
pub struct AuthState(Mutex<Option<(Session, Instant)>>);

impl AuthState {
    /// Create an empty authorization state.
    pub fn new() -> Self {
        Self::default()
    }

    /// Store or replace the current session.
    pub fn set_current_session(&self, user_id: i64, hostel_id: i64, role: impl Into<String>) {
        if let Ok(mut guard) = self.0.lock() {
            *guard = Some((
                Session {
                    user_id,
                    hostel_id,
                    role: role.into(),
                },
                Instant::now(),
            ));
        } else {
            error!("auth state mutex poisoned");
        }
    }

    /// Return the current session, if any, and reset its idle clock.
    ///
    /// A session whose last activity is older than [`SESSION_IDLE_TIMEOUT`]
    /// is cleared and treated as signed out.
    pub fn current_session(&self) -> Option<Session> {
        let mut guard = self.0.lock().ok()?;
        match guard.take() {
            Some((session, last_active)) if !is_idle(last_active, SESSION_IDLE_TIMEOUT) => {
                *guard = Some((session.clone(), Instant::now()));
                Some(session)
            }
            Some(_) => None,
            None => None,
        }
    }

    /// Clear the current session.
    pub fn clear_current_session(&self) {
        if let Ok(mut guard) = self.0.lock() {
            *guard = None;
        }
    }
}

fn is_idle(last_active: Instant, timeout: Duration) -> bool {
    last_active.elapsed() > timeout
}

/// Runtime license state for the desktop process: the last answer from the
/// server, if any. Data and mutating commands check it via
/// `commands::require_license` before doing any work.
#[derive(Debug, Default)]
pub struct LicenseState(Mutex<Option<AccountLicense>>);

impl LicenseState {
    /// Replace the cached license.
    pub fn set(&self, payload: AccountLicense) {
        if let Ok(mut guard) = self.0.lock() {
            *guard = Some(payload);
        } else {
            error!("license state mutex poisoned");
        }
    }

    /// The cached license, active or not.
    pub fn get(&self) -> Option<AccountLicense> {
        self.0.lock().ok().and_then(|g| g.clone())
    }

    /// The cached license if it is usable today.
    pub fn active(&self) -> Option<AccountLicense> {
        let today = chrono::Utc::now().date_naive();
        self.get().filter(|license| license.is_active(today))
    }

    /// Forget the license, for example when the server no longer knows this device.
    pub fn clear(&self) {
        if let Ok(mut guard) = self.0.lock() {
            *guard = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn license_works_offline_for_a_while_but_not_past_its_end_or_a_wound_back_clock() {
        let day = |d| NaiveDate::from_ymd_opt(2026, 11, d).unwrap();
        let license = AccountLicense {
            account_name: "Sunrise".into(),
            plan: "trial".into(),
            expires_on: day(30),
            checked_on: day(10),
            approved: true,
        };
        assert!(license.is_active(day(10)));
        let pending = AccountLicense {
            approved: false,
            ..license.clone()
        };
        assert!(
            !pending.is_active(day(10)),
            "waiting for the owner's approval"
        );
        assert!(license.is_active(day(24)), "14 days offline is fine");
        assert!(!license.is_active(day(25)), "15 days offline locks");
        assert!(
            !license.is_active(day(9)),
            "clock set before the last check"
        );
        let ended = AccountLicense {
            checked_on: day(30),
            ..license
        };
        assert!(ended.is_active(day(30)));
        assert!(
            !ended.is_active(NaiveDate::from_ymd_opt(2026, 12, 1).unwrap()),
            "past the plan's end"
        );
    }

    #[test]
    fn session_is_idle_after_timeout_elapses() {
        let just_now = Instant::now();
        assert!(!is_idle(just_now, Duration::from_secs(60)));

        let long_ago = Instant::now() - Duration::from_secs(120);
        assert!(is_idle(long_ago, Duration::from_secs(60)));
    }
}
