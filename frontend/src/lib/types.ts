/**
 * TypeScript mirrors of the Rust serde types (crates/db/src/model.rs,
 * crates/db/src/sync.rs, crates/app/src/{state,sync}.rs, crates/app/src/commands/*.rs
 * and the new IPC types in specs/v2.1.0-ui-rebuild.md).
 *
 * Fields are snake_case, exactly as serde writes them. Conventions:
 * - `Money` is a decimal string (`"150000.00"`). Never parse it into a number;
 *   show it with `formatMoney` and test it with `isOwing` (src/lib/money.ts).
 * - `IsoDate` is a `NaiveDate`: `"YYYY-MM-DD"`.
 * - `IsoDateTime` is a `DateTime<Utc>`: RFC 3339, e.g. `"2026-11-27T09:30:00Z"`.
 * - `Option<T>` is `T | null`. In requests, send `null` (not `undefined`) for "none".
 */

/** Decimal string. Display only; never do arithmetic on it in TypeScript. */
export type Money = string;
/** `NaiveDate`, `"YYYY-MM-DD"`. */
export type IsoDate = string;
/** `DateTime<Utc>`, RFC 3339. */
export type IsoDateTime = string;

/** Staff role. */
export type Role = 'admin' | 'manager';
/** Term of a stay. */
export type Term = 'semester_1' | 'semester_2' | 'recess';
/** How a payment was made. */
export type PaymentMethod = 'cash' | 'mobile_money' | 'bank_transfer' | 'card';

// ---------------------------------------------------------------- db models

/** `db::User`. The signed-in staff member (no password hash). */
export interface User {
  id: number;
  hostel_id: number;
  full_name: string;
  email: string;
  /** `"admin"` or `"manager"`. */
  role: Role | string;
  created_at: IsoDateTime;
}

/** `db::Hostel`. */
export interface Hostel {
  id: number;
  name: string;
  address: string | null;
  phone: string | null;
  /** ISO currency code, e.g. `"UGX"`. */
  currency: string;
  branding_logo_path: string | null;
  created_at: IsoDateTime;
}

/** `db::HostelProfile`: the hostel plus derived counts. */
export interface HostelProfile {
  id: number;
  name: string;
  address: string | null;
  phone: string | null;
  currency: string;
  branding_logo_path: string | null;
  room_count: number;
  admin_count: number;
  /** True when the hostel has at least one room and one admin. */
  onboarding_complete: boolean;
}

/** `db::AppPreferences`. */
export interface AppPreferences {
  hostel_id: number;
  date_format: string;
  language: string;
  receipt_footer: string | null;
  updated_at: IsoDateTime;
}

/** `db::RoomClass`: the type and prices a hostel's rooms share. */
export interface RoomClass {
  id: number;
  hostel_id: number;
  public_id: string;
  /** `"single"`, `"double"` or `"dorm"`. */
  room_type: string;
  semester_rate: Money;
  recess_rate: Money;
  /** How many rooms are in this class. */
  rooms: number;
}

/** `db::Room`, with its class's type and prices. Rooms have no resident limit. */
export interface Room {
  id: number;
  hostel_id: number;
  room_number: string;
  class_id: number;
  room_type: string;
  semester_rate: Money;
  recess_rate: Money;
  /** e.g. `"vacant"`, `"occupied"`, `"maintenance"`. */
  status: string;
  created_at: IsoDateTime;
  /** Residents with an active stay here. */
  residents: number;
}

/** `db::Resident`. */
export interface Resident {
  id: number;
  full_name: string;
  phone: string | null;
  email: string | null;
  id_number: string | null;
  next_of_kin: string | null;
  next_of_kin_phone: string | null;
  photo_path: string | null;
  created_at: IsoDateTime;
}

/** `db::ResidentSummary`: one row of the resident directory. */
export interface ResidentSummary {
  id: number;
  full_name: string;
  phone: string | null;
  email: string | null;
  /** Current active allocation, if any. */
  allocation_id: number | null;
  room_number: string | null;
  /** `"active"`, `"checked_out"` or `"never_allocated"`. */
  status: 'active' | 'checked_out' | 'never_allocated' | string;
  /** Owed minus paid on the active allocation, or `"0"`. */
  balance: Money;
}

/** `db::AllocationWithRoom`: a stay with its room number. */
export interface AllocationWithRoom {
  id: number;
  hostel_id: number;
  resident_id: number;
  room_id: number;
  /** `"active"` or `"checked_out"`. */
  status: 'active' | 'checked_out' | string;
  rate_agreed: Money;
  term: Term | string;
  /** e.g. `"2026/2027"`. */
  academic_year: string;
  created_at: IsoDateTime;
  room_number: string;
}

/** `db::Payment`. */
export interface Payment {
  id: number;
  hostel_id: number;
  allocation_id: number;
  resident_id: number;
  amount: Money;
  payment_date: IsoDateTime;
  payment_method: PaymentMethod | string;
  period_start: IsoDate | null;
  period_end: IsoDate | null;
  notes: string | null;
  created_at: IsoDateTime;
}

/** `db::ResidentDetail`: a resident with their stays and payments. */
export interface ResidentDetail {
  resident: Resident;
  allocations: AllocationWithRoom[];
  payments: Payment[];
  /** Net balance owed across all of the resident's stays. */
  balance: Money;
}

/** `db::PaymentRow`: one row of the payments ledger. */
export interface PaymentRow {
  id: number;
  resident_id: number;
  resident_name: string;
  allocation_id: number;
  room_number: string;
  amount: Money;
  payment_method: PaymentMethod | string;
  payment_date: IsoDateTime;
  receipt_number: string | null;
  notes: string | null;
  /** PDF written when the receipt was first printed on this desk. */
  pdf_path: string | null;
}

/** `db::ListPaymentsFilter`: the request of `list_payments`. */
export interface ListPaymentsFilter {
  hostel_id: number;
  from: IsoDate | null;
  to: IsoDate | null;
  resident_id: number | null;
  method: PaymentMethod | string | null;
}

/** `db::ActiveAllocation`: an active stay, for the payment form. */
export interface ActiveAllocation {
  allocation_id: number;
  resident_id: number;
  resident_name: string;
  room_number: string;
  rate_agreed: Money;
  term: Term | string;
  academic_year: string;
  total_paid: Money;
  /** Outstanding balance, clamped at zero by Rust. */
  balance: Money;
}

/** `db::DashboardStats`. */
export interface DashboardStats {
  total_rooms: number;
  occupied: number;
  vacant: number;
  maintenance: number;
  check_ins_today: number;
  revenue_today: Money;
  revenue_month: Money;
  outstanding_total: Money;
}

/** `db::ActivityItem`: one entry of the dashboard feed. */
export interface ActivityItem {
  kind: 'check_in' | 'check_out' | 'payment' | string;
  at: IsoDateTime;
  resident_name: string;
  room_number: string;
  /** Only set for payments. */
  amount: Money | null;
}

// ---------------------------------------------------------------- app state

/** `state::AccountLicense`: the server's last answer about the account. */
export interface AccountLicense {
  account_name: string;
  /** `"trial"`, `"semester"`, `"1-year"`, `"3-year"` or `"5-year"`. */
  plan: string;
  expires_on: IsoDate;
  checked_on: IsoDate;
  approved: boolean;
}

/** Which screen the app starts on. */
export type Gate = 'lock' | 'onboarding' | 'sign_in' | 'ready';

/** `boot_state` result. */
export interface BootState {
  gate: Gate;
  /** Set when `gate` is `"ready"`. */
  user: User | null;
  /** Onboarding: the hostel already came down from the cloud; only the admin is missing. */
  hostel_exists: boolean;
  /** This computer is connected to the cloud. */
  enrolled: boolean;
  /** Last license answer (usable or not). */
  license: AccountLicense | null;
  /** Why the computer is locked; shown on the lock screen when enrolled. */
  lock_reason: string | null;
  app_version: string;
}

/** `connect_cloud` result. */
export interface ConnectResult {
  licensed: boolean;
  lock_reason: string | null;
  /** Connected, but the first sync failed (it retries on its own). */
  sync_error: string | null;
}

/** Payload of the `sync` event and the result of `sync_now`. */
export interface SyncEvent {
  /** Toast text when synced changes need attention. Empty otherwise. */
  summary: string;
  /** The pass received data from the cloud: reload what is on screen. */
  changed: boolean;
  error: string | null;
  licensed: boolean;
}

/** `take_payment` result. */
export interface TakePaymentResult {
  /** The recorded payment; open its receipt with `desk.showReceipt`. */
  payment_id: number;
  receipt_number: string;
  /** The payment is recorded; only the PDF failed. */
  pdf_error: string | null;
}

// ---------------------------------------------------------------- requests

/** `users::CreateUserRequest`. */
export interface CreateUserRequest {
  hostel_id: number;
  full_name: string;
  email: string;
  password: string;
  role: Role;
}

/** `users::LoginRequest`. */
export interface LoginRequest {
  email: string;
  password: string;
}

/** `users::ListUsersRequest`. */
export interface ListUsersRequest {
  hostel_id: number;
}

/** `users::UpdateUserRequest`. `password` needs `current_password` when changing your own. */
export interface UpdateUserRequest {
  user_id: number;
  full_name: string;
  email: string;
  role: Role | string;
  password: string | null;
  current_password: string | null;
}

/** `users::DeleteUserRequest`. */
export interface DeleteUserRequest {
  user_id: number;
}

/** `hostels::CompleteOnboardingRequest`. */
export interface CompleteOnboardingRequest {
  hostel_name: string;
  address: string | null;
  phone: string | null;
  currency: string | null;
  branding_logo_path: string | null;
  admin_name: string;
  email: string;
  password: string;
}

/** `hostels::UpdateHostelRequest`. */
export interface UpdateHostelRequest {
  hostel_id: number;
  name: string;
  address: string | null;
  phone: string | null;
  currency: string;
  branding_logo_path: string | null;
}

/** `hostels::UploadHostelLogoRequest`. `data` is the file's bytes (`Vec<u8>` = array of numbers). */
export interface UploadHostelLogoRequest {
  hostel_id: number;
  data: number[];
  filename: string;
}

/** `rooms::SaveRoomClassRequest`. `class_id: null` creates a class. */
export interface SaveRoomClassRequest {
  hostel_id: number;
  class_id: number | null;
  room_type: string;
  semester_rate: Money;
  recess_rate: Money;
}

/** `rooms::AddRoomsRequest`: rooms of one class, added all or none. */
export interface AddRoomsRequest {
  hostel_id: number;
  class_id: number;
  room_numbers: string[];
}

/** `allocations::CheckInRequest`. Send `resident_id` for a returning resident, or `full_name` (+ details) for a new one. */
export interface CheckInRequest {
  hostel_id: number;
  room_id: number;
  resident_id: number | null;
  full_name: string | null;
  phone: string | null;
  email: string | null;
  id_number: string | null;
  next_of_kin: string | null;
  next_of_kin_phone: string | null;
  photo_path: string | null;
  rate_agreed: Money;
  term: Term;
  academic_year: string;
}

/** `allocations::CheckOutRequest`. */
export interface CheckOutRequest {
  hostel_id: number;
  allocation_id: number;
}

/** `allocations::ListActiveAllocationsRequest`. */
export interface ListActiveAllocationsRequest {
  hostel_id: number;
}

/** `take_payment` request: record a payment, print and open its receipt. */
export interface TakePaymentRequest {
  hostel_id: number;
  allocation_id: number;
  /** Decimal string typed by the user; Rust validates it. */
  amount: Money;
  payment_method: PaymentMethod;
  notes: string | null;
}

/** `open_receipt` request: reprint and open a past payment's receipt. */
export interface OpenReceiptRequest {
  hostel_id: number;
  payment_id: number;
}

/** `dashboard::GetDashboardStatsRequest`. */
export interface GetDashboardStatsRequest {
  hostel_id: number;
  /** Local date, `"YYYY-MM-DD"`. */
  today: IsoDate;
}

/** `dashboard::ListRecentActivityRequest`. */
export interface ListRecentActivityRequest {
  hostel_id: number;
  limit: number;
}

/** `residents::ListResidentsRequest`. */
export interface ListResidentsRequest {
  hostel_id: number;
}

/** `residents::GetResidentDetailRequest`. */
export interface GetResidentDetailRequest {
  hostel_id: number;
  resident_id: number;
}

/** `residents::UpdateResidentRequest`. */
export interface UpdateResidentRequest {
  hostel_id: number;
  resident_id: number;
  full_name: string;
  phone: string | null;
  email: string | null;
  id_number: string | null;
  next_of_kin: string | null;
  next_of_kin_phone: string | null;
}

/** `preferences::GetAppPreferencesRequest`. */
export interface GetAppPreferencesRequest {
  hostel_id: number;
}

/** `preferences::UpsertAppPreferencesRequest`. */
export interface UpsertAppPreferencesRequest {
  hostel_id: number;
  date_format: string;
  language: string;
  receipt_footer: string | null;
}

/** `connect_cloud` request. `server_url` blank = HostelDesk Cloud. */
export interface ConnectCloudRequest {
  server_url: string;
  email: string;
  password: string;
  create_account: boolean;
  /** Only used when `create_account` is true. */
  organization_name: string;
}
