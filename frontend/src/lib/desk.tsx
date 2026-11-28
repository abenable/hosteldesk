/**
 * DeskContext: everything a page needs about the signed-in session.
 * Provided by `<DeskProvider>` (src/components/DeskProvider.tsx) around the
 * AppShell; read it with `const desk = useDesk()`.
 *
 * Session rules (pages rely on these, do not work around them):
 * - The provider and everything under it are keyed by the user id and unmount
 *   on sign-out, session timeout (`"unauthorized"`) or lock (`"unlicensed"`), so
 *   no rows, form values or cached data of one session reach the next. Keep
 *   page state in components (useState/useLoad), never in module variables.
 * - Guard errors are handled for you: `"unauthorized"`/`"unlicensed"` re-run
 *   `boot_state` (back to Sign in / Lock), `"forbidden"` shows the toast
 *   "Only an administrator can do that." Pages just show `errorMessage(e)`.
 * - Writes: use `useAction` (src/lib/useLoad.ts) so a button or Enter can
 *   never submit twice; after a successful write call `desk.reload()`.
 * - Forms: seed form state once from loaded data and keep it local; a
 *   `dataVersion` bump must never overwrite what the user typed. See the
 *   "Forms and background reloads" note in src/lib/useLoad.ts.
 */
import { createContext, useContext, type ReactNode } from 'react';
import type { ToastInput } from './toast';
import type { AccountLicense, AppPreferences, HostelProfile, User } from './types';

/** Top-level sections, in sidebar order. */
export type Page = 'dashboard' | 'rooms' | 'residents' | 'payments' | 'settings';

/**
 * Where the shell is. No router: `desk.navigate({ page: 'residents', residentId: 7 })`
 * opens a resident; `tab` is free for a page's own sub-sections (e.g. settings tabs).
 */
export interface Route {
  page: Page;
  residentId?: number;
  tab?: string;
}

/** What `desk.confirm` asks. */
export interface ConfirmOptions {
  title: string;
  message?: ReactNode;
  /** The confirming verb: "Check out", "Remove", "Restore". */
  confirmLabel: string;
  /** `danger` (default) or `primary`. */
  tone?: 'danger' | 'primary';
  /**
   * The write to run on confirm. The dialog stays open with a spinner until it
   * settles; on failure it shows the error and stays open. Without `action`,
   * the dialog just closes and `confirm` resolves `true`.
   */
  action?: () => Promise<unknown>;
}

/** Background sync, as the top bar shows it. */
export interface SyncState {
  /** This computer syncs with the cloud. */
  enrolled: boolean;
  /** When the last pass succeeded (this session). */
  lastAt: Date | null;
  /** Last failure; cleared by the next success. The loop retries every 30 s. */
  error: string | null;
  /** A manual "sync now" is running. */
  busy: boolean;
}

/** The value of DeskContext. */
export interface DeskValue {
  /** The signed-in staff member. */
  user: User;
  hostelId: number;
  /** `user.role === 'admin'`. Hide admin-only actions for everyone else. */
  isAdmin: boolean;
  /** Hostel profile; reloaded on every `dataVersion` bump. */
  profile: HostelProfile;
  /** Hostel preferences (date format, receipt footer); reloaded with `profile`. */
  prefs: AppPreferences;
  /** The hostel's currency code (from the profile). Pass it to `formatMoney`. */
  currency: string;
  /** The usable license, or null (the desk is about to lock). */
  license: AccountLicense | null;
  appVersion: string;

  route: Route;
  navigate: (route: Route) => void;

  /** Show a toast (src/lib/toast.ts). */
  toast: (input: ToastInput) => void;
  /** Ask yes/no in a ConfirmDialog. Resolves `true` when confirmed (and `action`, if any, succeeded). */
  confirm: (options: ConfirmOptions) => Promise<boolean>;
  /** Show a payment's receipt in the in-app viewer. */
  showReceipt: (receipt: ReceiptRef) => void;

  /** Bumps after every sync that changed something and every `reload()`; `useLoad` re-fetches on it. */
  dataVersion: number;
  /** Re-fetch everything on screen (call after a successful write). */
  reload: () => void;

  /** `formatMoney(amount, currency)` for this hostel: `"UGX 1,500,000"`. */
  formatMoney: (amount: string) => string;
  /** A date or timestamp in the hostel's date format. */
  formatDate: (value: string) => string;
  /** Date plus local time, `"27/11/2026 09:30"`. */
  formatDateTime: (value: string) => string;

  sync: SyncState;
  /** Run a sync pass now (e.g. a "Sync now" button). */
  syncNow: () => void;
  /** End the session and go back to Sign in. */
  signOut: () => void;
}

/** A receipt to show: the payment it belongs to and its number (for the title). */
export interface ReceiptRef {
  paymentId: number;
  receiptNumber: string;
}

/** The context object. `null` outside the signed-in shell (gate screens). */
export const DeskContext = createContext<DeskValue | null>(null);

/** The signed-in session. Only valid inside the AppShell (every page). */
export function useDesk(): DeskValue {
  const desk = useContext(DeskContext);
  if (!desk) throw new Error('useDesk() is only available inside <DeskProvider>.');
  return desk;
}
