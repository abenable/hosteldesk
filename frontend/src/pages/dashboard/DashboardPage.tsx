import {
  Alert,
  Avatar,
  Button,
  Card,
  cx,
  EmptyState,
  PageHeader,
  Skeleton,
  Stat,
  StatGroup,
} from '../../components/ui';
import * as api from '../../lib/api';
import { useDesk } from '../../lib/desk';
import { todayIso } from '../../lib/format';
import type { ActivityItem, Room } from '../../lib/types';
import { useLoad, type Load } from '../../lib/useLoad';
import { OccupancyBar } from '../rooms/OccupancyBar';
import { bedTotals, freeBeds, percent, roomState, type RoomState } from '../rooms/room';

/** How many feed entries to show (Rust caps it at 50). */
const ACTIVITY_LIMIT = 8;
/** Room tiles before "View all" takes over. */
const TILE_LIMIT = 24;

const KIND: Record<string, string> = {
  check_in: 'Checked in',
  check_out: 'Checked out',
  payment: 'Paid',
};

const RELATIVE = new Intl.RelativeTimeFormat('en', { numeric: 'auto' });

/** "just now", "5 minutes ago", "yesterday"; a week or older → the hostel's date format. */
function relativeTime(at: string, formatDate: (value: string) => string): string {
  const seconds = Math.round((new Date(at).getTime() - Date.now()) / 1000);
  const size = Math.abs(seconds);
  if (Number.isNaN(size)) return at;
  if (size < 60) return 'just now';
  if (size < 3_600) return RELATIVE.format(Math.round(seconds / 60), 'minute');
  if (size < 86_400) return RELATIVE.format(Math.round(seconds / 3_600), 'hour');
  if (size < 7 * 86_400) return RELATIVE.format(Math.round(seconds / 86_400), 'day');
  return formatDate(at);
}

/** `"2026-11-27"` → `"Friday 27 November 2026"`. */
function longDate(iso: string): string {
  const [y = 0, m = 1, d = 1] = iso.split('-').map(Number);
  return new Date(y, m - 1, d).toLocaleDateString('en-GB', {
    weekday: 'long',
    day: 'numeric',
    month: 'long',
    year: 'numeric',
  });
}

/** Pulsing block in place of a figure that is still loading. */
const pending = <span className="block h-8 w-28 animate-pulse rounded-md bg-gray-200" />;

/**
 * Today at a glance (ui.rs `dashboard`): beds and takings in a stat strip,
 * the recent-activity feed, and a bed map of the rooms. Quick actions jump to
 * the check-in and payment flows on their pages.
 */
export default function DashboardPage() {
  const desk = useDesk();
  const today = todayIso();
  const stats = useLoad(
    () => api.dashboardStats({ hostel_id: desk.hostelId, today }),
    [desk.hostelId, today],
  );
  const activity = useLoad(
    () => api.recentActivity({ hostel_id: desk.hostelId, limit: ACTIVITY_LIMIT }),
    [desk.hostelId],
  );
  const rooms = useLoad(() => api.rooms(desk.hostelId), [desk.hostelId]);

  const beds = rooms.data && bedTotals(rooms.data);
  const checkIns = stats.data?.check_ins_today;
  const subtitle = [
    longDate(today),
    beds && beds.total > 0 && `${beds.occupied} of ${beds.total} beds occupied`,
    checkIns !== undefined && `${checkIns} ${checkIns === 1 ? 'check-in' : 'check-ins'} today`,
  ]
    .filter(Boolean)
    .join(' · ');

  return (
    <>
      <PageHeader
        title="Dashboard"
        description={subtitle}
        actions={
          <>
            <Button
              variant="secondary"
              icon="banknotes"
              onClick={() => desk.navigate({ page: 'payments', tab: 'outstanding' })}
            >
              Take payment
            </Button>
            <Button icon="user-plus" onClick={() => desk.navigate({ page: 'residents', tab: 'check-in' })}>
              Check in
            </Button>
          </>
        }
      />

      {stats.error && !stats.data ? (
        <Alert title="Could not load today's figures" onRetry={stats.reload}>
          {stats.error}
        </Alert>
      ) : (
        <StatGroup>
          {beds && beds.total > 0 ? (
            <Stat
              label="Beds occupied"
              value={`${beds.occupied} / ${beds.total}`}
              badge={{ text: `${percent(beds.occupied, beds.total)}%` }}
              hint={`${beds.total - beds.occupied} free`}
            />
          ) : (
            <Stat label="Beds occupied" value={beds ? '0' : rooms.loading ? pending : '—'} hint={beds && 'No rooms yet'} />
          )}
          <Stat
            label="Collected today"
            value={stats.data ? desk.formatMoney(stats.data.revenue_today) : pending}
            hint={stats.data && `${desk.formatMoney(stats.data.revenue_month)} this month`}
          />
          <Stat
            label="Outstanding"
            value={stats.data ? desk.formatMoney(stats.data.outstanding_total) : pending}
            hint={stats.data && 'Still owed on current stays'}
          />
        </StatGroup>
      )}

      <div className="grid items-start gap-6 xl:grid-cols-2">
        <Card
          title="Recent activity"
          description="Check-ins, check-outs and payments, newest first."
          flush
        >
          <Activity load={activity} canCheckIn={!!rooms.data?.length} />
        </Card>
        <Card
          title="Rooms"
          description={
            rooms.data && rooms.data.length > 0
              ? `${rooms.data.filter((r) => roomState(r) === 'available').length} of ${rooms.data.length} with a free bed`
              : undefined
          }
          actions={
            rooms.data &&
            rooms.data.length > 0 && (
              <Button variant="ghost" size="sm" onClick={() => desk.navigate({ page: 'rooms' })}>
                View all
              </Button>
            )
          }
        >
          <RoomMap load={rooms} />
        </Card>
      </div>
    </>
  );
}

/** The activity feed (HyperUI details-list rows): who, what, room, amount, when. */
function Activity({ load, canCheckIn }: { load: Load<ActivityItem[]>; canCheckIn: boolean }) {
  const desk = useDesk();
  if (load.loading)
    return (
      <div className="px-6 pb-6">
        <Skeleton rows={6} />
      </div>
    );
  if (!load.data)
    return (
      <div className="px-6 pb-6">
        <Alert title="Could not load recent activity" onRetry={load.reload}>
          {load.error}
        </Alert>
      </div>
    );
  if (load.data.length === 0)
    return (
      <div className="border-t border-gray-200">
        <EmptyState
          icon="inbox"
          title="No activity yet"
          description="Check-ins, check-outs and payments will show up here as they happen."
          action={
            canCheckIn && (
              <Button
                variant="secondary"
                icon="user-plus"
                onClick={() => desk.navigate({ page: 'residents', tab: 'check-in' })}
              >
                Check in a resident
              </Button>
            )
          }
        />
      </div>
    );
  return (
    <ul className="divide-y divide-gray-200 border-t border-gray-200">
      {load.data.map((item, i) => (
        <li key={`${item.kind}-${item.at}-${i}`} className="flex items-center gap-4 px-6 py-3.5">
          <Avatar name={item.resident_name} size="sm" />
          <div className="min-w-0 flex-1">
            <p className="truncate text-sm font-medium text-gray-900">{item.resident_name}</p>
            <p className="truncate text-sm text-gray-500">
              {KIND[item.kind] ?? item.kind.replace(/_/g, ' ')} · Room {item.room_number}
            </p>
          </div>
          <div className="shrink-0 text-end text-sm">
            {item.amount !== null && (
              <p className="font-medium text-gray-900 tabular-nums">{desk.formatMoney(item.amount)}</p>
            )}
            <time dateTime={item.at} title={desk.formatDateTime(item.at)} className="text-gray-500">
              {relativeTime(item.at, desk.formatDate)}
            </time>
          </div>
        </li>
      ))}
    </ul>
  );
}

const TILE_TONE: Record<RoomState, string> = {
  available: 'text-green-700',
  full: 'text-gray-500',
  maintenance: 'text-amber-700',
};

/** Compact bed map: one tile per room, its occupied beds and what is free. */
function RoomMap({ load }: { load: Load<Room[]> }) {
  const desk = useDesk();
  if (load.loading) return <Skeleton rows={5} />;
  if (!load.data)
    return (
      <Alert title="Could not load rooms" onRetry={load.reload}>
        {load.error}
      </Alert>
    );
  if (load.data.length === 0)
    return (
      <EmptyState
        icon="home"
        title="No rooms yet"
        description={
          desk.isAdmin
            ? 'Add your rooms to start checking residents in.'
            : 'An administrator adds the rooms.'
        }
        action={
          desk.isAdmin && (
            <Button icon="plus" onClick={() => desk.navigate({ page: 'rooms', tab: 'add' })}>
              Add room
            </Button>
          )
        }
      />
    );
  const more = load.data.length - TILE_LIMIT;
  return (
    <>
      <ul className="grid grid-cols-3 gap-2">
        {load.data.slice(0, TILE_LIMIT).map((room) => {
          const state = roomState(room);
          return (
            <li
              key={room.id}
              title={`Room ${room.room_number}: ${room.occupied_beds} of ${room.capacity} beds occupied${state === 'maintenance' ? ', under maintenance' : ''}`}
              className="rounded-lg border border-gray-200 px-3 py-2.5"
            >
              <p className="flex items-baseline justify-between gap-2 text-sm">
                <span className="truncate font-medium text-gray-900">{room.room_number}</span>
                <span className={cx('text-xs whitespace-nowrap tabular-nums', TILE_TONE[state])}>
                  {state === 'available' ? `${freeBeds(room)} free` : state === 'full' ? 'Full' : 'Closed'}
                </span>
              </p>
              <OccupancyBar room={room} className="mt-2" />
            </li>
          );
        })}
      </ul>
      {more > 0 && (
        <p className="mt-3 text-sm text-gray-500">
          and {more} more {more === 1 ? 'room' : 'rooms'}
        </p>
      )}
    </>
  );
}
