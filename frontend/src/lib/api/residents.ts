import { invoke } from '@tauri-apps/api/core';
import { reportError } from './support.ts';
import type { ResidentDetail, ResidentSummary, UpdateResidentInput } from './types.ts';

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

export async function updateResident(input: UpdateResidentInput): Promise<void> {
  try {
    return await invoke<void>('update_resident', { request: input });
  } catch (error) {
    reportError(error);
    throw error;
  }
}
