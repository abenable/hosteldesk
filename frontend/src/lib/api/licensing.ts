import { invoke } from '@tauri-apps/api/core';
import { reportError } from './support.ts';
import type { LicensePayload } from './types.ts';

export async function getAppVersion(): Promise<string> {
  try {
    return await invoke<string>('get_app_version');
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
