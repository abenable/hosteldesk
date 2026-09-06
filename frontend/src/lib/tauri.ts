import { invoke } from '@tauri-apps/api/core';
import { addToast } from './toasts.ts';

// Invoke convention: top-level argument keys are snake_case. Commands that take
// bare parameters are annotated with rename_all = "snake_case" on the Rust
// side; commands that take a single `request` struct pass it as `{ request }`.

export type LicensePayload = {
  hostel_name: string;
  expiry_date: string;
  tier: string;
};

export type Room = {
  id: number;
  hostel_id: number;
  room_number: string;
  room_type: string;
  capacity: number;
  floor: number | null;
  semester_rate: string;
  recess_rate: string;
  status: 'vacant' | 'occupied' | 'maintenance' | string;
  created_at: string;
  occupied_beds: number;
};

export type Term = 'semester_1' | 'semester_2' | 'recess';

export type UserInput = {
  hostel_id: number;
  full_name: string;
  email: string;
  password: string;
  role: string;
};

export type HostelInput = {
  name: string;
  address?: string;
  phone?: string;
  currency?: string;
  branding_logo_path?: string;
};

export type Hostel = {
  id: number;
  name: string;
  address?: string;
  phone?: string;
  currency: string;
  branding_logo_path?: string;
  created_at: string;
};

export type HostelProfile = {
  id: number;
  name: string;
  address?: string;
  phone?: string;
  currency: string;
  branding_logo_path?: string;
  room_count: number;
  admin_count: number;
  onboarding_complete: boolean;
};

export type User = {
  id: number;
  hostel_id: number;
  full_name: string;
  email: string;
  role: string;
  created_at: string;
};

export type UpdateUserInput = {
  user_id: number;
  full_name: string;
  email: string;
  role: string;
  password?: string;
};

export type LoginInput = {
  email: string;
  password: string;
};

function toMessage(error: unknown): string {
  if (error instanceof Error) return error.message;
  return typeof error === 'string' ? error : String(error);
}

function reportError(error: unknown) {
  addToast(toMessage(error), 'error');
}

export async function getAppVersion(): Promise<string> {
  try {
    return await invoke<string>('get_app_version');
  } catch (error) {
    reportError(error);
    throw error;
  }
}

export async function getRooms(hostelId: number): Promise<Room[]> {
  try {
    return await invoke<Room[]>('get_rooms', { hostel_id: hostelId });
  } catch (error) {
    reportError(error);
    throw error;
  }
}

export type CheckInInput = {
  hostel_id: number;
  room_id: number;
  resident_id?: number | null;
  full_name?: string | null;
  phone?: string | null;
  email?: string | null;
  id_number?: string | null;
  next_of_kin?: string | null;
  photo_path?: string | null;
  check_in_date: string;
  check_out_date?: string | null;
  expected_checkout: string;
  rate_agreed: string;
  term: Term;
  academic_year: string;
};

export type CompleteOnboardingInput = {
  hostel_name: string;
  address?: string | null;
  phone?: string | null;
  currency?: string | null;
  branding_logo_path?: string | null;
  admin_name: string;
  email: string;
  password: string;
};

export async function checkIn(input: CheckInInput): Promise<number> {
  try {
    return await invoke<number>('check_in', { request: input });
  } catch (error) {
    reportError(error);
    throw error;
  }
}

export async function getLicenseStatus(): Promise<LicensePayload | null> {
  return invoke<LicensePayload | null>('get_license_status');
}

export async function activateLicenseFromFile(): Promise<LicensePayload> {
  return invoke<LicensePayload>('activate_license_from_file');
}

export async function createUser(input: UserInput): Promise<number> {
  try {
    return await invoke<number>('create_user', { request: input });
  } catch (error) {
    reportError(error);
    throw error;
  }
}

export async function login(input: LoginInput): Promise<User> {
  try {
    return await invoke<User>('login', { request: input });
  } catch (error) {
    reportError(error);
    throw error;
  }
}

export async function getOnboardingStatus(): Promise<boolean> {
  try {
    return await invoke<boolean>('get_onboarding_status');
  } catch (error) {
    reportError(error);
    throw error;
  }
}

export async function completeOnboarding(
  input: CompleteOnboardingInput,
): Promise<User> {
  try {
    return await invoke<User>('complete_onboarding', { request: input });
  } catch (error) {
    reportError(error);
    throw error;
  }
}

export async function getSession(): Promise<User | null> {
  try {
    return await invoke<User | null>('get_session');
  } catch (error) {
    reportError(error);
    throw error;
  }
}

export async function logout(): Promise<void> {
  try {
    await invoke<void>('logout');
  } catch (error) {
    reportError(error);
    throw error;
  }
}

export async function listHostels(): Promise<Hostel[]> {
  try {
    return await invoke<Hostel[]>('list_hostels');
  } catch (error) {
    reportError(error);
    throw error;
  }
}

export async function getHostelProfile(hostelId: number): Promise<HostelProfile | null> {
  try {
    return await invoke<HostelProfile | null>('get_hostel_profile', {
      hostel_id: hostelId,
    });
  } catch (error) {
    reportError(error);
    throw error;
  }
}

export type AddRoomInput = {
  hostel_id: number;
  room_number: string;
  room_type: string;
  capacity: number;
  floor?: number | null;
  semester_rate: string;
  recess_rate: string;
};

export async function addRoom(input: AddRoomInput): Promise<number> {
  try {
    return await invoke<number>('add_room', { request: input });
  } catch (error) {
    reportError(error);
    throw error;
  }
}

export type PaymentInput = {
  hostel_id: number;
  allocation_id: number;
  resident_id: number;
  amount: string;
  payment_method: string;
  period_start?: string | null;
  period_end?: string | null;
  notes?: string | null;
};

export async function recordPaymentWithReceipt(
  input: PaymentInput,
): Promise<[number, string]> {
  try {
    return await invoke<[number, string]>('record_payment_and_generate_receipt', {
      request: input,
    });
  } catch (error) {
    reportError(error);
    throw error;
  }
}

export type ReceiptInput = {
  hostel_id: number;
  hostel_name: string;
  hostel_address?: string | null;
  hostel_phone?: string | null;
  receipt_number: number;
  resident_name: string;
  room_number: string;
  amount: string;
  date: string;
  balance: string;
  payment_method: string;
  notes?: string | null;
  currency_code: string;
  footer_text?: string | null;
  logo_path?: string | null;
};

export async function generateAndOpenReceipt(input: ReceiptInput): Promise<string> {
  try {
    return await invoke<string>('generate_and_open_receipt', { request: input });
  } catch (error) {
    reportError(error);
    throw error;
  }
}

export type Resident = {
  id: number;
  full_name: string;
  phone?: string;
  email?: string;
  id_number?: string;
  next_of_kin?: string;
  photo_path?: string;
  created_at: string;
};

export type Payment = {
  id: number;
  hostel_id: number;
  allocation_id: number;
  resident_id: number;
  amount: string;
  payment_date: string;
  payment_method: string;
  period_start?: string | null;
  period_end?: string | null;
  notes?: string;
  created_at: string;
};

export type ResidentSummary = {
  id: number;
  full_name: string;
  phone?: string;
  email?: string;
  allocation_id?: number;
  room_number?: string;
  check_in_date?: string;
  expected_checkout?: string;
  status: 'active' | 'checked_out' | 'never_allocated' | string;
  balance: string;
};

export type AllocationWithRoom = {
  id: number;
  hostel_id: number;
  resident_id: number;
  room_id: number;
  check_in_date: string;
  check_out_date?: string | null;
  expected_checkout: string;
  status: string;
  rate_agreed: string;
  term: Term;
  academic_year: string;
  created_at: string;
  room_number: string;
};

export type ResidentDetail = {
  resident: Resident;
  allocations: AllocationWithRoom[];
  payments: Payment[];
  balance: string;
};

export type PaymentRow = {
  id: number;
  resident_id: number;
  resident_name: string;
  allocation_id: number;
  room_number: string;
  amount: string;
  payment_method: string;
  payment_date: string;
  receipt_number?: string;
  notes?: string;
};

export type ListPaymentsFilter = {
  hostel_id: number;
  from?: string | null;
  to?: string | null;
  resident_id?: number;
  method?: string;
};

export type ActiveAllocation = {
  allocation_id: number;
  resident_id: number;
  resident_name: string;
  room_number: string;
  rate_agreed: string;
  term: Term;
  academic_year: string;
  check_in_date: string;
  expected_checkout: string;
  total_paid: string;
  balance: string;
};

export type DashboardStats = {
  total_rooms: number;
  occupied: number;
  vacant: number;
  maintenance: number;
  check_ins_today: number;
  departures_today: number;
  overdue_stays: number;
  revenue_today: string;
  revenue_month: string;
  outstanding_total: string;
};

export type ActivityItem = {
  kind: 'check_in' | 'check_out' | 'payment' | string;
  at: string;
  resident_name: string;
  room_number: string;
  amount?: string;
};

export type UpdateHostelInput = {
  hostel_id: number;
  name: string;
  address?: string;
  phone?: string;
  currency: string;
  branding_logo_path?: string;
};

export type UploadHostelLogoInput = {
  hostel_id: number;
  data: number[];
  filename: string;
};

export type AppPreferences = {
  hostel_id: number;
  date_format: string;
  language: string;
  receipt_footer?: string;
  updated_at: string;
};

export type UpsertAppPreferencesInput = {
  hostel_id: number;
  date_format: string;
  language: string;
  receipt_footer?: string | null;
};

export type RestoreBackupInput = {
  backup_path: string;
};

export type UpdateResidentInput = {
  hostel_id: number;
  resident_id: number;
  full_name: string;
  phone?: string;
  email?: string;
  id_number?: string;
  next_of_kin?: string;
};

export async function listResidents(input: { hostel_id: number }): Promise<ResidentSummary[]> {
  try {
    return await invoke<ResidentSummary[]>('list_residents', { request: input });
  } catch (error) {
    reportError(error);
    throw error;
  }
}

export async function getResidentDetail(input: {
  hostel_id: number;
  resident_id: number;
}): Promise<ResidentDetail> {
  try {
    return await invoke<ResidentDetail>('get_resident_detail', { request: input });
  } catch (error) {
    reportError(error);
    throw error;
  }
}

export async function checkOut(input: {
  hostel_id: number;
  allocation_id: number;
  check_out_date: string;
}): Promise<void> {
  try {
    return await invoke<void>('check_out', { request: input });
  } catch (error) {
    reportError(error);
    throw error;
  }
}

export async function listPayments(input: ListPaymentsFilter): Promise<PaymentRow[]> {
  try {
    return await invoke<PaymentRow[]>('list_payments', { request: input });
  } catch (error) {
    reportError(error);
    throw error;
  }
}

export async function listActiveAllocations(input: {
  hostel_id: number;
}): Promise<ActiveAllocation[]> {
  try {
    return await invoke<ActiveAllocation[]>('list_active_allocations', { request: input });
  } catch (error) {
    reportError(error);
    throw error;
  }
}

export async function getDashboardStats(input: {
  hostel_id: number;
  today: string;
}): Promise<DashboardStats> {
  try {
    return await invoke<DashboardStats>('get_dashboard_stats', { request: input });
  } catch (error) {
    reportError(error);
    throw error;
  }
}

export async function listRecentActivity(input: {
  hostel_id: number;
  limit: number;
}): Promise<ActivityItem[]> {
  try {
    return await invoke<ActivityItem[]>('list_recent_activity', { request: input });
  } catch (error) {
    reportError(error);
    throw error;
  }
}

export async function updateHostel(input: UpdateHostelInput): Promise<Hostel> {
  try {
    return await invoke<Hostel>('update_hostel', { request: input });
  } catch (error) {
    reportError(error);
    throw error;
  }
}

export async function updateResident(input: UpdateResidentInput): Promise<void> {
  try {
    return await invoke<void>('update_resident', { request: input });
  } catch (error) {
    reportError(error);
    throw error;
  }
}

export async function listUsers(input: { hostel_id: number }): Promise<User[]> {
  try {
    return await invoke<User[]>('list_users', { request: input });
  } catch (error) {
    reportError(error);
    throw error;
  }
}

export async function updateUser(input: UpdateUserInput): Promise<User> {
  try {
    return await invoke<User>('update_user', { request: input });
  } catch (error) {
    reportError(error);
    throw error;
  }
}

export async function backupDatabase(): Promise<string> {
  try {
    return await invoke<string>('backup_database');
  } catch (error) {
    reportError(error);
    throw error;
  }
}

export async function deleteUser(input: { user_id: number }): Promise<void> {
  try {
    return await invoke<void>('delete_user', { request: input });
  } catch (error) {
    reportError(error);
    throw error;
  }
}

export async function uploadHostelLogo(input: UploadHostelLogoInput): Promise<string> {
  try {
    return await invoke<string>('upload_hostel_logo', { request: input });
  } catch (error) {
    reportError(error);
    throw error;
  }
}

export async function listBackups(): Promise<string[]> {
  try {
    return await invoke<string[]>('list_backups');
  } catch (error) {
    reportError(error);
    throw error;
  }
}

export async function restoreBackup(input: RestoreBackupInput): Promise<void> {
  try {
    return await invoke<void>('restore_backup', { request: input });
  } catch (error) {
    reportError(error);
    throw error;
  }
}

export async function getAppPreferences(input: { hostel_id: number }): Promise<AppPreferences> {
  try {
    return await invoke<AppPreferences>('get_app_preferences', { request: input });
  } catch (error) {
    reportError(error);
    throw error;
  }
}

export async function updateAppPreferences(input: UpsertAppPreferencesInput): Promise<AppPreferences> {
  try {
    return await invoke<AppPreferences>('update_app_preferences', { request: input });
  } catch (error) {
    reportError(error);
    throw error;
  }
}
