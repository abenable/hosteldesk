import { invoke } from '@tauri-apps/api/core';
import { reportError } from './support.ts';
import type {
  CompleteOnboardingInput,
  Hostel,
  HostelProfile,
  UpdateHostelInput,
  UploadHostelLogoInput,
  User,
} from './types.ts';

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

export async function updateHostel(input: UpdateHostelInput): Promise<Hostel> {
  try {
    return await invoke<Hostel>('update_hostel', { request: input });
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
