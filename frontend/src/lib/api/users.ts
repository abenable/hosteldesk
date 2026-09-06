import { invoke } from '@tauri-apps/api/core';
import { reportError } from './support.ts';
import type { LoginInput, UpdateUserInput, User, UserInput } from './types.ts';

export async function createUser(input: UserInput): Promise<number> {
  try {
    return await invoke<number>('create_user', { request: input });
  } catch (error) {
    reportError(error);
    throw error;
  }
}

export async function login(input: LoginInput): Promise<User> {
  try {
    return await invoke<User>('login', { request: input });
  } catch (error) {
    reportError(error);
    throw error;
  }
}

export async function getSession(): Promise<User | null> {
  try {
    return await invoke<User | null>('get_session');
  } catch (error) {
    reportError(error);
    throw error;
  }
}

export async function logout(): Promise<void> {
  try {
    await invoke<void>('logout');
  } catch (error) {
    reportError(error);
    throw error;
  }
}

export async function listUsers(input: { hostel_id: number }): Promise<User[]> {
  try {
    return await invoke<User[]>('list_users', { request: input });
  } catch (error) {
    reportError(error);
    throw error;
  }
}

export async function updateUser(input: UpdateUserInput): Promise<User> {
  try {
    return await invoke<User>('update_user', { request: input });
  } catch (error) {
    reportError(error);
    throw error;
  }
}

export async function deleteUser(input: { user_id: number }): Promise<void> {
  try {
    return await invoke<void>('delete_user', { request: input });
  } catch (error) {
    reportError(error);
    throw error;
  }
}
