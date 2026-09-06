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

export type CheckInInput = {
  hostel_id: number;
  room_id: number;
  resident_id?: number | null;
  full_name?: string | null;
  phone?: string | null;
  email?: string | null;
  id_number?: string | null;
  next_of_kin?: string | null;
  next_of_kin_phone?: string | null;
  photo_path?: string | null;
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

export type AddRoomInput = {
  hostel_id: number;
  room_number: string;
  room_type: string;
  capacity: number;
  floor?: number | null;
  semester_rate: string;
  recess_rate: string;
};

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

export type Resident = {
  id: number;
  full_name: string;
  phone?: string;
  email?: string;
  id_number?: string;
  next_of_kin?: string;
  next_of_kin_phone?: string;
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
  status: 'active' | 'checked_out' | 'never_allocated' | string;
  balance: string;
};

export type AllocationWithRoom = {
  id: number;
  hostel_id: number;
  resident_id: number;
  room_id: number;
  status: string;
  rate_agreed: string;
  term: Term;
  academic_year: string;
  created_at: string;
  updated_at: string;
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
  total_paid: string;
  balance: string;
};

export type DashboardStats = {
  total_rooms: number;
  occupied: number;
  vacant: number;
  maintenance: number;
  check_ins_today: number;
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
  next_of_kin_phone?: string;
};
