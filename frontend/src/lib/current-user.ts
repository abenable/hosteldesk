import type { StaffUser } from './tauri.ts';

const KEY = 'hosteldesk.current_user';

/**
 * Return the cached current user from localStorage for display purposes.
 *
 * This value is not authoritative for access control. All authorization
 * decisions are enforced by the backend session state.
 */
export function getCurrentUser(): StaffUser | null {
  try {
    const raw = localStorage.getItem(KEY);
    return raw ? (JSON.parse(raw) as StaffUser) : null;
  } catch {
    return null;
  }
}

/**
 * Cache the current user in localStorage so the UI can show name/role.
 *
 * This is a display-only copy. Mutating commands are gated server-side by
 * the in-memory session created at login.
 */
export function setCurrentUser(user: StaffUser | null) {
  if (!user) {
    localStorage.removeItem(KEY);
  } else {
    localStorage.setItem(KEY, JSON.stringify(user));
  }
}
