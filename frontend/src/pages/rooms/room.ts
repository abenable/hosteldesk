/**
 * Room facts shared by the Rooms page, the setup wizard and the dashboard.
 * Rooms have no resident limit: any number of residents may share one.
 */
import type { BadgeTone } from '../../components/ui';
import type { Room, RoomClass } from '../../lib/types';

/** Room types, in form order (the only values `room_classes` accepts). */
export const ROOM_TYPES = [
  { value: 'single', label: 'Single' },
  { value: 'double', label: 'Double' },
  { value: 'dorm', label: 'Dorm' },
];

/** `"dorm"` → `"Dorm"`. Unknown values are shown as given. */
export const typeLabel = (type: string) => ROOM_TYPES.find((t) => t.value === type)?.label ?? type;

/** `"Single · Floor 2"`, or `"Single"` with no floor: names a class (or a room's class). */
export const classLabel = (c: Pick<RoomClass, 'room_type' | 'floor'>) =>
  c.floor === null ? typeLabel(c.room_type) : `${typeLabel(c.room_type)} · Floor ${c.floor}`;

/** Vacant (nobody), occupied (someone stays), or under maintenance (no check-ins). */
export type RoomState = 'vacant' | 'occupied' | 'maintenance';

/** Maintenance wins; otherwise anyone staying makes the room occupied. */
export function roomState(room: Room): RoomState {
  if (room.status === 'maintenance') return 'maintenance';
  return room.residents > 0 ? 'occupied' : 'vacant';
}

/** Badge for each state: green vacant, grey occupied, amber maintenance. */
export const STATE_BADGE: Record<RoomState, { label: string; tone: BadgeTone }> = {
  vacant: { label: 'Vacant', tone: 'success' },
  occupied: { label: 'Occupied', tone: 'neutral' },
  maintenance: { label: 'Maintenance', tone: 'warning' },
};

/** Whole percent of `part` in `whole`; 0 when there is nothing to count. */
export const percent = (part: number, whole: number) =>
  whole > 0 ? Math.round((part * 100) / whole) : 0;

/** At most this many rooms per add, so a typo in a range cannot make thousands. */
export const MAX_ROOMS = 500;

/**
 * Room numbers `prefix + from … prefix + to`. A leading zero in `from` pads
 * every number to its width ("01" to "20" gives A01 … A20). Returns an error
 * message instead when the range is not usable.
 */
export function roomRange(prefix: string, from: string, to: string): string[] | string {
  if (!/^\d{1,4}$/.test(from.trim()) || !/^\d{1,4}$/.test(to.trim()))
    return 'Use whole numbers, like 1 to 20.';
  const first = Number(from.trim());
  const last = Number(to.trim());
  if (last < first) return 'The last number cannot be below the first.';
  if (last - first + 1 > MAX_ROOMS) return `Add at most ${MAX_ROOMS} rooms at a time.`;
  const width = from.trim().startsWith('0') ? from.trim().length : 0;
  return Array.from({ length: last - first + 1 }, (_, i) =>
    `${prefix.trim()}${String(first + i).padStart(width, '0')}`,
  );
}

/** Room numbers typed as a list: split on commas, spaces or new lines, repeats dropped. */
export const roomList = (text: string) => [
  ...new Set(
    text
      .split(/[\s,]+/)
      .map((n) => n.trim())
      .filter(Boolean),
  ),
];

/** `"A01, A02, A03 … A20"`: the first few numbers and the last. */
export const preview = (numbers: string[]) =>
  numbers.length <= 6 ? numbers.join(', ') : `${numbers.slice(0, 3).join(', ')} … ${numbers.at(-1)}`;
