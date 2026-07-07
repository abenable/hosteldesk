import { invoke } from '@tauri-apps/api/core';
import { addToast } from './toasts.ts';

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
  rate_per_period: string;
  status: 'vacant' | 'occupied' | 'maintenance' | string;
  created_at: string;
};

export type ResidentInput = {
  full_name: string;
  phone?: string;
  email?: string;
  id_number?: string;
  next_of_kin?: string;
  photo_path?: string;
};

export type AllocationInput = {
  hostel_id: number;
  resident_id: number;
  room_id: number;
  bed_id?: number | null;
  check_in_date: string;
  check_out_date?: string | null;
  expected_checkout: string;
  rate_agreed: string;
};

export type StaffUserInput = {
  hostel_id: number;
  name: string;
  username: string;
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

export type StaffUser = {
  id: number;
  hostel_id: number;
  name: string;
  username: string;
  role: string;
  created_at: string;
};

export type LoginInput = {
  username: string;
  password: string;
};

function toMessage(error: unknown): string {
  if (error instanceof Error) return error.message;
  return typeof error === 'string' ? error : String(error);
}

function reportError(error: unknown) {
  addToast(toMessage(error), 'error');
}

export async function getRooms(hostelId = 1): Promise<Room[]> {
  try {
    return await invoke<Room[]>('get_rooms', { hostel_id: hostelId });
  } catch (error) {
    reportError(error);
    throw error;
  }
}

export async function addResident(input: ResidentInput): Promise<number> {
  try {
    return await invoke<number>('add_resident', { request: input });
  } catch (error) {
    reportError(error);
    throw error;
  }
}

export async function createAllocation(input: AllocationInput): Promise<number> {
  try {
    return await invoke<number>('create_allocation', { request: input });
  } catch (error) {
    reportError(error);
    throw error;
  }
}

export async function getLicenseStatus(): Promise<LicensePayload | null> {
  return invoke<LicensePayload | null>('get_license_status');
}

export async function activateLicense(license: string): Promise<LicensePayload> {
  return invoke<LicensePayload>('activate_license', { license });
}

export async function generateTrialLicense(): Promise<string> {
  return invoke<string>('generate_trial_license', { hostel_name: 'Trial Hostel' });
}

export async function createStaffUser(input: StaffUserInput): Promise<number> {
  try {
    return await invoke<number>('create_staff_user', { request: input });
  } catch (error) {
    reportError(error);
    throw error;
  }
}

export async function login(input: LoginInput): Promise<StaffUser> {
  try {
    return await invoke<StaffUser>('login', { request: input });
  } catch (error) {
    reportError(error);
    throw error;
  }
}

export async function createHostel(input: HostelInput): Promise<Hostel> {
  try {
    return await invoke<Hostel>('create_hostel', {
      request: { ...input, currency: input.currency ?? 'UGX' },
    });
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

export type AddRoomInput = {
  hostel_id: number;
  room_number: string;
  room_type: string;
  capacity: number;
  floor?: number | null;
  rate_per_period: string;
};

export async function addRoom(input: AddRoomInput): Promise<number> {
  try {
    return await invoke<number>('add_room', { request: input });
  } catch (error) {
    reportError(error);
    throw error;
  }
}

export async function bulkAddRooms(input: AddRoomInput[]): Promise<number[]> {
  try {
    return await invoke<number[]>('bulk_add_rooms', { requests: input });
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
  hostel_name: string;
  receipt_number: number;
  resident_name: string;
  amount: string;
  date: string;
  balance: string;
};

export async function generateAndOpenReceipt(input: ReceiptInput): Promise<string> {
  try {
    return await invoke<string>('generate_and_open_receipt', { request: input });
  } catch (error) {
    reportError(error);
    throw error;
  }
}
