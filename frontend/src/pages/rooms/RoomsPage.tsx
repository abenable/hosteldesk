import { useState } from 'react';
import {
  Alert,
  Avatar,
  Badge,
  Button,
  Card,
  DetailPanel,
  DetailSection,
  EmptyState,
  MiniStats,
  PageHeader,
  Skeleton,
  Table,
  Tabs,
  type Column,
} from '../../components/ui';
import * as api from '../../lib/api';
import { useDesk } from '../../lib/desk';
import { termLabel } from '../../lib/format';
import { isOwing } from '../../lib/money';
import type { Room } from '../../lib/types';
import { useLoad } from '../../lib/useLoad';
import { AddRoomModal } from './AddRoomModal';
import { OccupancyBar } from './OccupancyBar';
import { bedTotals, freeBeds, roomState, STATE_BADGE, typeLabel } from './room';

type Filter = 'all' | 'available' | 'full';

const NONE: Record<Exclude<Filter, 'all'>, { title: string; description: string }> = {
  available: { title: 'No free beds', description: 'Every room is full right now.' },
  full: { title: 'No full rooms', description: 'Every room has at least one free bed.' },
};

/** Rooms, beds and rates (ui.rs `rooms` + `add_room`), with the selected room in a DetailPanel. */
export default function RoomsPage() {
  const desk = useDesk();
  const { data: rooms, error, loading, reload } = useLoad(() => api.rooms(desk.hostelId), [desk.hostelId]);
  const [filter, setFilter] = useState<Filter>('all');
  const [selectedId, setSelectedId] = useState<number | null>(null);
  // The dashboard's "Add room" lands here with tab 'add'. A new key per opening starts the form empty.
  const [adding, setAdding] = useState(desk.isAdmin && desk.route.tab === 'add');
  const [addKey, setAddKey] = useState(0);
  const openAdd = () => {
    setAddKey((k) => k + 1);
    setAdding(true);
  };

  const add = desk.isAdmin && (
    <Button variant="secondary" icon="plus" onClick={openAdd}>
      Add room
    </Button>
  );
  const modal = desk.isAdmin && (
    <AddRoomModal
      key={addKey}
      open={adding}
      onClose={() => setAdding(false)}
      onAdded={(id) => {
        setAdding(false);
        setFilter('all');
        setSelectedId(id);
      }}
    />
  );

  if (!rooms) {
    return (
      <>
        <PageHeader title="Rooms" description="Rooms, beds and rates." actions={add} />
        {loading ? (
          <Card>
            <Skeleton rows={8} />
          </Card>
        ) : (
          <Alert title="Could not load rooms" onRetry={reload}>
            {error}
          </Alert>
        )}
        {modal}
      </>
    );
  }

  const beds = bedTotals(rooms);
  const available = rooms.filter((r) => roomState(r) === 'available');
  const full = rooms.filter((r) => roomState(r) === 'full');
  const shown = filter === 'available' ? available : filter === 'full' ? full : rooms;
  const selected = rooms.find((r) => r.id === selectedId);

  const room: Column<Room> = {
    header: 'Room',
    cell: (r) => (
      <>
        <p className="font-medium text-gray-900">{r.room_number}</p>
        <p className="text-sm font-normal text-gray-500">
          {typeLabel(r.room_type)}
          {r.floor !== null && ` · Floor ${r.floor}`}
        </p>
      </>
    ),
  };
  const bedsColumn: Column<Room> = {
    header: 'Beds',
    cell: (r) => (
      <div className="w-20">
        <p className="tabular-nums">
          {r.occupied_beds} of {r.capacity}
        </p>
        <OccupancyBar room={r} className="mt-1.5" />
      </div>
    ),
  };
  const status: Column<Room> = {
    header: 'Status',
    cell: (r) => {
      const badge = STATE_BADGE[roomState(r)];
      return <Badge tone={badge.tone}>{badge.label}</Badge>;
    },
  };
  const semester: Column<Room> = { header: 'Semester', cell: (r) => desk.formatMoney(r.semester_rate), align: 'end' };
  const recess: Column<Room> = { header: 'Recess', cell: (r) => desk.formatMoney(r.recess_rate), align: 'end' };
  // Floor rides on the room's second line so the table fits at 1024 px. With the
  // DetailPanel open the table is narrower still; the recess rate is in the panel.
  const columns = selected
    ? [room, bedsColumn, status, semester]
    : [room, bedsColumn, status, semester, recess];

  return (
    <>
      <PageHeader
        title="Rooms"
        description={
          rooms.length === 0
            ? 'Rooms, beds and rates.'
            : `${rooms.length} ${rooms.length === 1 ? 'room' : 'rooms'} · ${beds.occupied} of ${beds.total} beds occupied`
        }
        actions={add}
      />
      {rooms.length === 0 ? (
        <Card>
          <EmptyState
            icon="home"
            title="No rooms yet"
            description={
              desk.isAdmin
                ? 'Add your rooms with their beds and rates to start checking residents in.'
                : 'An administrator adds the rooms. They will show up here.'
            }
            action={
              desk.isAdmin && (
                <Button icon="plus" onClick={openAdd}>
                  Add room
                </Button>
              )
            }
          />
        </Card>
      ) : (
        <>
          {error && (
            <Alert tone="warning" title="Showing the last rooms loaded" onRetry={reload}>
              {error}
            </Alert>
          )}
          <Tabs
            label="Room filter"
            value={filter}
            onChange={setFilter}
            items={[
              { value: 'all', label: 'All', count: rooms.length },
              { value: 'available', label: 'Available', count: available.length },
              { value: 'full', label: 'Full', count: full.length },
            ]}
          />
          <Card flush>
            {shown.length === 0 && filter !== 'all' ? (
              <EmptyState icon="home" title={NONE[filter].title} description={NONE[filter].description} />
            ) : (
              <Table
                key={filter}
                caption="Rooms"
                columns={columns}
                rows={shown}
                rowKey={(r) => r.id}
                selectedKey={selected?.id}
                onRowClick={(r) => setSelectedId(r.id)}
                pageSize={15}
                noun="rooms"
              />
            )}
          </Card>
        </>
      )}
      {selected && <RoomDetail room={selected} onClose={() => setSelectedId(null)} />}
      {modal}
    </>
  );
}

/** The selected room: beds, rates and who is staying there (from `active_allocations`). */
function RoomDetail({ room, onClose }: { room: Room; onClose: () => void }) {
  const desk = useDesk();
  const stays = useLoad(() => api.activeAllocations({ hostel_id: desk.hostelId }), [desk.hostelId]);
  const state = roomState(room);
  const badge = STATE_BADGE[state];
  const here = stays.data?.filter((s) => s.room_number === room.room_number) ?? [];
  const floor = room.floor === null ? 'No floor set' : `Floor ${room.floor}`;

  return (
    <DetailPanel
      title={`Room ${room.room_number}`}
      subtitle={`${typeLabel(room.room_type)} · ${floor}`}
      onClose={onClose}
      footer={
        <Button
          className="flex-1"
          icon="user-plus"
          disabled={state !== 'available'}
          onClick={() => desk.navigate({ page: 'residents', tab: 'check-in' })}
        >
          Check in a resident
        </Button>
      }
    >
      <div className="flex items-center gap-2">
        <Badge tone={badge.tone}>{badge.label}</Badge>
        <span className="text-sm text-gray-500">
          {state === 'available'
            ? `${freeBeds(room)} of ${room.capacity} beds free`
            : state === 'full'
              ? 'No free beds'
              : 'Not taking check-ins'}
        </span>
      </div>
      <MiniStats
        items={[
          { label: 'Beds', value: room.capacity },
          { label: 'In use', value: room.occupied_beds },
          { label: 'Free', value: freeBeds(room) },
        ]}
      />
      <DetailSection label="Rates">
        <dl className="divide-y divide-gray-200">
          {(
            [
              ['Semester', room.semester_rate],
              ['Recess', room.recess_rate],
            ] as const
          ).map(([label, rate]) => (
            <div key={label} className="flex items-center justify-between gap-4 py-2 first:pt-0 last:pb-0">
              <dt className="text-gray-600">{label}</dt>
              <dd className="font-medium tabular-nums">{desk.formatMoney(rate)}</dd>
            </div>
          ))}
        </dl>
      </DetailSection>
      <DetailSection label="Staying here">
        {stays.loading ? (
          <Skeleton rows={2} />
        ) : stays.error && !stays.data ? (
          <Alert title="Could not load residents" onRetry={stays.reload}>
            {stays.error}
          </Alert>
        ) : here.length === 0 ? (
          <p className="text-gray-500">Nobody is staying in this room.</p>
        ) : (
          <ul className="-mx-2">
            {here.map((s) => (
              <li key={s.allocation_id}>
                <button
                  type="button"
                  onClick={() => desk.navigate({ page: 'residents', residentId: s.resident_id })}
                  className="flex w-full items-center gap-3 rounded-lg px-2 py-2 text-start transition-colors hover:bg-gray-50 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-primary-900"
                >
                  <Avatar name={s.resident_name} size="sm" />
                  <span className="min-w-0 flex-1">
                    <span className="block truncate font-medium text-gray-900">{s.resident_name}</span>
                    <span className="block truncate text-gray-500">
                      {termLabel(s.term)} · {s.academic_year}
                    </span>
                  </span>
                  {isOwing(s.balance) ? (
                    <span className="text-end text-sm font-medium text-red-700 tabular-nums">
                      {desk.formatMoney(s.balance)}
                      <span className="block text-xs font-normal text-gray-500">owing</span>
                    </span>
                  ) : (
                    <Badge tone="success">Paid</Badge>
                  )}
                </button>
              </li>
            ))}
          </ul>
        )}
      </DetailSection>
    </DetailPanel>
  );
}
