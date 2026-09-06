use std::sync::Mutex;
use std::time::{Duration, Instant};

use license::LicensePayload;
use tracing::error;

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

/// Runtime license state managed by Tauri.
///
/// `None` means the app is locked because no license was supplied or the
/// stored license is invalid/expired. Data/mutating commands check this state
/// via `commands::require_license` before doing any work.
#[derive(Debug, Default)]
pub struct LicenseState(Mutex<Option<LicensePayload>>);

impl LicenseState {
    /// Replace the current license payload.
    pub fn set(&self, payload: LicensePayload) {
        if let Ok(mut guard) = self.0.lock() {
            *guard = Some(payload);
        } else {
            error!("license state mutex poisoned");
        }
    }

    /// Return the currently active license payload, if any.
    pub fn get(&self) -> Option<LicensePayload> {
        self.0.lock().ok().and_then(|g| g.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_is_idle_after_timeout_elapses() {
        let just_now = Instant::now();
        assert!(!is_idle(just_now, Duration::from_secs(60)));

        let long_ago = Instant::now() - Duration::from_secs(120);
        assert!(is_idle(long_ago, Duration::from_secs(60)));
    }
}
