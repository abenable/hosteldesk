/**
 * Room facts shared by the Rooms page and the dashboard. Bed counts are plain
 * integers (not money), so counting them here is fine.
 */
import type { BadgeTone } from '../../components/ui';
import type { Room } from '../../lib/types';

/** Room types, in form order (the only values `insert_room` accepts). */
export const ROOM_TYPES = [
  { value: 'single', label: 'Single' },
  { value: 'double', label: 'Double' },
  { value: 'dorm', label: 'Dorm' },
];

/** `"dorm"` → `"Dorm"`. Unknown values are shown as given. */
export const typeLabel = (type: string) => ROOM_TYPES.find((t) => t.value === type)?.label ?? type;

/** Beds still free. A room has free beds only when `occupied_beds < capacity`, whatever `status` says. */
export const freeBeds = (room: Room) => Math.max(room.capacity - room.occupied_beds, 0);

/** What the desk cares about: can someone be checked in here? */
export type RoomState = 'available' | 'full' | 'maintenance';

/** Maintenance wins; otherwise a free bed makes the room available. */
export function roomState(room: Room): RoomState {
  if (room.status === 'maintenance') return 'maintenance';
  return freeBeds(room) > 0 ? 'available' : 'full';
}

/** Badge for each state: green available, red full, amber maintenance. */
export const STATE_BADGE: Record<RoomState, { label: string; tone: BadgeTone }> = {
  available: { label: 'Available', tone: 'success' },
  full: { label: 'Full', tone: 'danger' },
  maintenance: { label: 'Maintenance', tone: 'warning' },
};

/** Beds across all rooms: `{ total, occupied }`. */
export function bedTotals(rooms: Room[]) {
  let total = 0;
  let occupied = 0;
  for (const room of rooms) {
    total += room.capacity;
    occupied += Math.min(room.occupied_beds, room.capacity);
  }
  return { total, occupied };
}

/** Whole percent of `part` in `whole`; 0 when there is nothing to count. */
export const percent = (part: number, whole: number) =>
  whole > 0 ? Math.round((part * 100) / whole) : 0;
