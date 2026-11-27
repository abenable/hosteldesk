/**
 * Helpers shared by the residents screens: free rooms for check-in (ports of
 * `room_choice` / `rate_for_term` in main's ui.rs) and the check-out
 * confirmation (`Confirm::CheckOut`).
 */
import { useCallback } from 'react';
import * as api from '../../lib/api';
import { useDesk } from '../../lib/desk';
import { isOwing } from '../../lib/money';
import type { Room } from '../../lib/types';

/** Rooms a resident can be checked into: not under maintenance and with a free bed. */
export const freeRooms = (rooms: Room[]) =>
  rooms.filter((room) => room.status !== 'maintenance' && room.occupied_beds < room.capacity);

/** `"12 · double · 1 of 2 beds free"` (ui.rs `room_choice`). */
export const roomChoice = (room: Room) =>
  `${room.room_number} · ${room.room_type} · ${room.capacity - room.occupied_beds} of ${room.capacity} beds free`;

/** The room's rate for `term`: recess has its own rate (ui.rs `rate_for_term`). */
export const rateForTerm = (room: Room, term: string) =>
  term === 'recess' ? room.recess_rate : room.semester_rate;

/** What the check-out confirmation needs about a stay. */
export interface CheckOutStay {
  allocation_id: number;
  name: string;
  /** What the stay still owes (`ActiveAllocation.balance`). */
  balance: string;
}

/**
 * Returns `checkOut(stay)`: asks "Check out <name>?" (warning when the stay
 * still owes, as ui.rs did), checks out on confirm, then toasts and reloads.
 */
export function useCheckOut() {
  const { confirm, formatMoney, hostelId, reload, toast } = useDesk();
  return useCallback(
    async (stay: CheckOutStay) => {
      const done = await confirm({
        title: `Check out ${stay.name}?`,
        message: isOwing(stay.balance)
          ? `${stay.name} still owes ${formatMoney(stay.balance)}. Check out anyway?`
          : `${stay.name} has paid in full.`,
        confirmLabel: 'Check out',
        tone: 'danger',
        action: () => api.checkOut({ hostel_id: hostelId, allocation_id: stay.allocation_id }),
      });
      if (!done) return;
      toast({ tone: 'success', title: 'Checked out.', message: stay.name });
      reload();
    },
    [confirm, formatMoney, hostelId, reload, toast],
  );
}
