// Floor-letter room numbering: floors lettered A-first (ground = A),
// rooms zero-padded to 2 digits. A01, A02, ..., B01.

const LETTERS = 'ABCDEFGHIJKLMNOPQRSTUVWXYZ';

/** 0-based floor index → letter (0 → A). */
export function floorLetter(index: number): string {
  if (index < 0) return 'A';
  return LETTERS[index % 26] ?? 'A';
}

/** Letter → 0-based floor index (A → 0). Defaults to 0. */
export function floorIndex(letter: string): number {
  const i = LETTERS.indexOf(letter.toUpperCase());
  return i < 0 ? 0 : i;
}

/** Floor letter + 1-based ordinal → room number, e.g. ("A", 1) → "A01". */
export function formatRoomNumber(letter: string, n: number): string {
  return `${letter.toUpperCase()}${String(n).padStart(2, '0')}`;
}

/** Floors to offer in a select (A–H by default). */
export const FLOOR_OPTIONS = Array.from({ length: 8 }, (_, i) => floorLetter(i));

/** Group rooms by floor letter, sorted A-first. */
export function groupByFloor<T extends { room_number: string }>(
  rooms: T[],
): { floor: string; rooms: T[] }[] {
  const map = new Map<string, T[]>();
  for (const r of rooms) {
    const letter = (r.room_number.match(/^[A-Za-z]+/) ?? ['?'])[0].toUpperCase();
    const list = map.get(letter) ?? [];
    list.push(r);
    map.set(letter, list);
  }
  return Array.from(map.entries())
    .sort(([a], [b]) => a.localeCompare(b))
    .map(([floor, rs]) => ({ floor, rooms: rs }));
}
