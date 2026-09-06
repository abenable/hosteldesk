import { invoke } from '@tauri-apps/api/core';
import { reportError } from './support.ts';
import type { RestoreBackupInput } from './types.ts';

export async function backupDatabase(): Promise<string> {
  try {
    return await invoke<string>('backup_database');
  } catch (error) {
    reportError(error);
    throw error;
  }
}

export async function listBackups(): Promise<string[]> {
  try {
    return await invoke<string[]>('list_backups');
  } catch (error) {
    reportError(error);
    throw error;
  }
}

export async function restoreBackup(input: RestoreBackupInput): Promise<void> {
  try {
    return await invoke<void>('restore_backup', { request: input });
  } catch (error) {
    reportError(error);
    throw error;
  }
}
