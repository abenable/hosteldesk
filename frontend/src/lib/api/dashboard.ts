import { invoke } from '@tauri-apps/api/core';
import { reportError } from './support.ts';
import type { ActivityItem, DashboardStats } from './types.ts';

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
