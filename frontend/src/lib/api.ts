/**
 * The only module that talks to Rust. One typed function per Tauri command
 * (specs/v2.1.0-ui-rebuild.md, "IPC contract").
 *
 * - Commands that take a request struct send it as `{ request }` with snake_case fields.
 * - Commands that take a bare scalar send it camelCase (`hostel_profile(hostel_id)` → `{ hostelId }`).
 * - Every function rejects with an {@link ApiError}; show `errorMessage(e)` to the user.
 * - The guard errors `"unauthorized"` / `"unlicensed"` / `"forbidden"` are also
 *   reported to the handler App installs with {@link onGuardError} (re-boot / toast),
 *   so pages never need to special-case them.
 */
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import type * as T from './types';

/** The stable guard errors from `crates/app/src/commands/support.rs`. */
export type GuardCode = 'unauthorized' | 'unlicensed' | 'forbidden';

const GUARD_TEXT: Record<GuardCode, string> = {
  unauthorized: 'Your session timed out. Sign in again.',
  unlicensed: 'This computer needs a valid HostelDesk license.',
  forbidden: 'Only an administrator can do that.',
};

/** A failed command. `message` is safe to show; `guard` is set for the three guard errors. */
export class ApiError extends Error {
  readonly guard: GuardCode | null;

  constructor(raw: string) {
    const guard = raw in GUARD_TEXT ? (raw as GuardCode) : null;
    super(guard ? GUARD_TEXT[guard] : raw);
    this.name = 'ApiError';
    this.guard = guard;
  }
}

/** Text to show for anything a promise rejected with. */
export function errorMessage(error: unknown): string {
  if (error instanceof Error) return error.message;
  if (typeof error === 'string') return error;
  return 'Something went wrong.';
}

let guardHandler: ((code: GuardCode) => void) | null = null;

/** Install the app-wide guard handler (App does this once). Returns an uninstall function. */
export function onGuardError(handler: (code: GuardCode) => void): () => void {
  guardHandler = handler;
  return () => {
    if (guardHandler === handler) guardHandler = null;
  };
}

async function call<R>(command: string, args?: Record<string, unknown>): Promise<R> {
  try {
    return await invoke<R>(command, args);
  } catch (raw) {
    const error = new ApiError(errorMessage(raw));
    if (error.guard) guardHandler?.(error.guard);
    throw error;
  }
}

/** Subscribe to the background `sync` event. Returns an unsubscribe function. */
export function onSync(handler: (event: T.SyncEvent) => void): () => void {
  let stop: (() => void) | null = null;
  let stopped = false;
  listen<T.SyncEvent>('sync', (event) => handler(event.payload)).then(
    (unlisten) => {
      if (stopped) unlisten();
      else stop = unlisten;
    },
    (error: unknown) => console.error('Could not listen for sync events', error),
  );
  return () => {
    stopped = true;
    stop?.();
  };
}

// ------------------------------------------------------------ gate & session

/** Which screen to show, and everything the gate screens need. */
export const bootState = () => call<T.BootState>('boot_state');
/** Sign in to (or create) a HostelDesk account, enroll this computer, then sync once. */
export const connectCloud = (request: T.ConnectCloudRequest) =>
  call<T.ConnectResult>('connect_cloud', { request });
/** Run one sync pass now (the same pass the 30 s background loop runs). */
export const syncNow = () => call<T.SyncEvent>('sync_now');
/** This computer is connected to a HostelDesk account (also in `bootState().enrolled`). */
export const cloudEnrolled = () => call<boolean>('cloud_enrolled');
/** The usable license, or null when this computer is locked. */
export const licenseStatus = () => call<T.AccountLicense | null>('license_status');
/** The server's last answer about the license, usable or not. */
export const cachedLicense = () => call<T.AccountLicense | null>('cached_license');
/** Version of the running app, e.g. `"2.1.0"`. */
export const appVersion = () => call<string>('app_version');
/** The signed-in staff member, if the session is still alive. */
export const session = () => call<T.User | null>('session');
/** End the staff session. */
export const logout = () => call<void>('logout');
/** Sign a staff member in. */
export const login = (request: T.LoginRequest) => call<T.User>('login', { request });
/** Whether the first hostel and admin exist. */
export const onboardingStatus = () => call<boolean>('onboarding_status');
/** Create the hostel (unless it came from the cloud) and its first admin; signs them in. */
export const completeOnboarding = (request: T.CompleteOnboardingRequest) =>
  call<T.User>('complete_onboarding', { request });

// ------------------------------------------------------------ staff

/** Add a staff account. Admin only. Returns the new user id. */
export const createUser = (request: T.CreateUserRequest) => call<number>('create_user', { request });
/** Staff of a hostel. Admin only. */
export const listUsers = (request: T.ListUsersRequest) => call<T.User[]>('list_users', { request });
/** Update a staff profile or password. */
export const updateUser = (request: T.UpdateUserRequest) => call<T.User>('update_user', { request });
/** Remove a staff account. Admin only; never the last admin. */
export const deleteUser = (request: T.DeleteUserRequest) => call<void>('delete_user', { request });

// ------------------------------------------------------------ hostel

/** The hostel profile with room/admin counts. */
export const hostelProfile = (hostelId: number) =>
  call<T.HostelProfile | null>('hostel_profile', { hostelId });
/** Hostels this session may see. */
export const listHostels = () => call<T.Hostel[]>('list_hostels');
/** Save the business profile. Admin only. */
export const updateHostel = (request: T.UpdateHostelRequest) =>
  call<T.Hostel>('update_hostel', { request });
/** Store a logo image; returns its path for `updateHostel`. Admin only. */
export const uploadHostelLogo = (request: T.UploadHostelLogoRequest) =>
  call<string>('upload_hostel_logo', { request });

// ------------------------------------------------------------ rooms & stays

/** Rooms of a hostel. */
export const rooms = (hostelId: number) => call<T.Room[]>('rooms', { hostelId });
/** Room classes of a hostel. */
export const roomClasses = (hostelId: number) => call<T.RoomClass[]>('room_classes', { hostelId });
/** Create (`class_id: null`) or edit a room class. Admin only. Returns its id. */
export const saveRoomClass = (request: T.SaveRoomClassRequest) =>
  call<number>('save_room_class', { request });
/** Add rooms of one class, all or none. Admin only. Returns their ids. */
export const addRooms = (request: T.AddRoomsRequest) => call<number[]>('add_rooms', { request });
/** Open the email app on a renewal request to HostelDesk (fixed address, built in Rust). */
export const emailRenewal = () => call<void>('email_renewal');
/** Check a resident in. Returns the new allocation id. */
export const checkIn = (request: T.CheckInRequest) => call<number>('check_in', { request });
/** Check an allocation out. */
export const checkOut = (request: T.CheckOutRequest) => call<void>('check_out', { request });
/** Active stays with what each has paid and still owes. */
export const activeAllocations = (request: T.ListActiveAllocationsRequest) =>
  call<T.ActiveAllocation[]>('active_allocations', { request });

// ------------------------------------------------------------ payments

/** Record a payment and write its receipt PDF. Use this for the payment form. */
export const takePayment = (request: T.TakePaymentRequest) =>
  call<T.TakePaymentResult>('take_payment', { request });
/** A payment's receipt PDF (printed again when this computer has none), for the in-app viewer. */
export const receiptPdf = (request: T.OpenReceiptRequest) =>
  call<ArrayBuffer>('receipt_pdf', { request });
/** Open a payment's receipt in the computer's own PDF app. */
export const openReceipt = (request: T.OpenReceiptRequest) =>
  call<void>('open_receipt', { request });
/** The payments ledger, newest first. */
export const listPayments = (request: T.ListPaymentsFilter) =>
  call<T.PaymentRow[]>('list_payments', { request });

// ------------------------------------------------------------ dashboard

/** Occupancy and revenue figures for `today`. */
export const dashboardStats = (request: T.GetDashboardStatsRequest) =>
  call<T.DashboardStats>('dashboard_stats', { request });
/** Recent check-ins, check-outs and payments. */
export const recentActivity = (request: T.ListRecentActivityRequest) =>
  call<T.ActivityItem[]>('recent_activity', { request });

// ------------------------------------------------------------ residents

/** The resident directory. */
export const listResidents = (request: T.ListResidentsRequest) =>
  call<T.ResidentSummary[]>('list_residents', { request });
/** One resident with stays and payments. */
export const residentDetail = (request: T.GetResidentDetailRequest) =>
  call<T.ResidentDetail>('resident_detail', { request });
/** Save a resident's contact details. */
export const updateResident = (request: T.UpdateResidentRequest) =>
  call<void>('update_resident', { request });

// ------------------------------------------------------------ settings

/** Preferences of a hostel (defaults when never saved). */
export const appPreferences = (request: T.GetAppPreferencesRequest) =>
  call<T.AppPreferences>('app_preferences', { request });
/** Save preferences. Admin only. */
export const updateAppPreferences = (request: T.UpsertAppPreferencesRequest) =>
  call<T.AppPreferences>('update_app_preferences', { request });
