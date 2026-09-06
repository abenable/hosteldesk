import { invoke } from '@tauri-apps/api/core';
import { reportError } from './support.ts';
import type { AppPreferences, UpsertAppPreferencesInput } from './types.ts';

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
