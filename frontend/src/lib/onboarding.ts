// ponytail: local-only until create_hostel/add_room commands land

const KEY = 'hosteldesk.onboarding';

export type HostelProfile = {
  name: string;
  address?: string;
  phone?: string;
};

export type StaffRole = 'admin' | 'manager';

export type AdminAccount = {
  email: string;
  phone: string;
  name: string;
  role: StaffRole;
};

type OnboardingState = {
  complete: boolean;
  hostel?: HostelProfile;
  admin?: AdminAccount;
};

function read(): OnboardingState {
  try {
    const raw = localStorage.getItem(KEY);
    return raw ? (JSON.parse(raw) as OnboardingState) : { complete: false };
  } catch {
    return { complete: false };
  }
}

function write(state: OnboardingState) {
  localStorage.setItem(KEY, JSON.stringify(state));
}

export function isOnboardingComplete(): boolean {
  return read().complete === true;
}

export function getHostelProfile(): HostelProfile | undefined {
  return read().hostel;
}

export function saveHostelProfile(profile: HostelProfile) {
  const state = read();
  write({ ...state, hostel: profile });
}

export function saveAdminAccount(admin: AdminAccount) {
  const state = read();
  write({ ...state, admin });
}

export function completeOnboarding() {
  const state = read();
  write({ ...state, complete: true });
  window.dispatchEvent(new Event('hosteldesk:onboarding-complete'));
}

export function resetOnboarding() {
  write({ complete: false });
  window.dispatchEvent(new Event('hosteldesk:onboarding-reset'));
}
