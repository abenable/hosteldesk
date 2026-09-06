import { invoke } from '@tauri-apps/api/core';
import { reportError } from './support.ts';
import type { AddRoomInput, Room } from './types.ts';

export async function getRooms(hostelId: number): Promise<Room[]> {
  try {
    return await invoke<Room[]>('get_rooms', { hostel_id: hostelId });
  } catch (error) {
    reportError(error);
    throw error;
  }
}

export async function addRoom(input: AddRoomInput): Promise<number> {
  try {
    return await invoke<number>('add_room', { request: input });
  } catch (error) {
    reportError(error);
    throw error;
  }
}
