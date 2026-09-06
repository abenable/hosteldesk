import { invoke } from '@tauri-apps/api/core';
import { reportError } from './support.ts';
import type { ActiveAllocation, CheckInInput } from './types.ts';

export async function checkIn(input: CheckInInput): Promise<number> {
  try {
    return await invoke<number>('check_in', { request: input });
  } catch (error) {
    reportError(error);
    throw error;
  }
}

export async function checkOut(input: {
  hostel_id: number;
  allocation_id: number;
}): Promise<void> {
  try {
    return await invoke<void>('check_out', { request: input });
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
