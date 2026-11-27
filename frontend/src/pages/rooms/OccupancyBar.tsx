import { cx } from '../../components/ui';
import type { Room } from '../../lib/types';
import { percent, roomState } from './room';

/**
 * Thin bar of occupied beds in a room (amber under maintenance). Decorative: always
 * put the figures ("2 of 3") next to it. Candidate for the kit as a Meter.
 */
export function OccupancyBar({ room, className }: { room: Room; className?: string }) {
  const width = Math.min(100, percent(room.occupied_beds, room.capacity));
  return (
    <span
      aria-hidden="true"
      className={cx('block h-1.5 overflow-hidden rounded-full bg-gray-200', className)}
    >
      <span
        className={cx(
          'block h-full rounded-full',
          // Neutral fill: the status badge (or the tile's label) carries the colour.
          roomState(room) === 'maintenance' ? 'bg-amber-500' : 'bg-gray-900',
        )}
        style={{ width: `${width}%` }}
      />
    </span>
  );
}
