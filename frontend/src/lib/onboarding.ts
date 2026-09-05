import type { StaffUser } from './tauri.ts';

const KEY = 'hosteldesk.onboarding';

type OnboardingState = {
  complete: boolean;
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

export function completeOnboarding(user?: StaffUser) {
  const state = read();
  write({ ...state, complete: true });
  window.dispatchEvent(
    new CustomEvent('hosteldesk:onboarding-complete', { detail: user }),
  );
}

export function resetOnboarding() {
  write({ complete: false });
  window.dispatchEvent(new Event('hosteldesk:onboarding-reset'));
}
