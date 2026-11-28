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
  Modal,
  PageHeader,
  SearchInput,
  Select,
  Skeleton,
  Table,
  Tabs,
  type Column,
} from '../../components/ui';
import * as api from '../../lib/api';
import { useDesk } from '../../lib/desk';
import { termLabel } from '../../lib/format';
import { isOwing } from '../../lib/money';
import type { Room, RoomClass } from '../../lib/types';
import { useLoad } from '../../lib/useLoad';
import { ClassForm } from './ClassForm';
import { ClassList, RoomSetup } from './RoomSetup';
import { classLabel, ROOM_TYPES, roomState, STATE_BADGE } from './room';

type Filter = 'all' | 'vacant' | 'occupied';

const NONE: Record<Exclude<Filter, 'all'>, { title: string; description: string }> = {
  vacant: { title: 'No vacant rooms', description: 'Someone is staying in every room.' },
  occupied: { title: 'No occupied rooms', description: 'Nobody is staying in any room yet.' },
};

/**
 * Rooms and their classes, with the selected room in a DetailPanel. Rooms
 * filter by state (tabs), room number and, when there is more than one, type. An
 * administrator adds rooms (and classes) through the setup wizard, which
 * also opens from the dashboard (tab `add`) and here (tab `setup`).
 */
export default function RoomsPage() {
  const desk = useDesk();
  const { data: rooms, error, loading, reload } = useLoad(() => api.rooms(desk.hostelId), [desk.hostelId]);
  const classes = useLoad(() => api.roomClasses(desk.hostelId), [desk.hostelId]);
  const [filter, setFilter] = useState<Filter>('all');
  const [query, setQuery] = useState('');
  const [type, setType] = useState('');
  const [selectedId, setSelectedId] = useState<number | null>(null);
  const [editing, setEditing] = useState<RoomClass | null>(null);
  const setup = desk.isAdmin && (desk.route.tab === 'setup' || desk.route.tab === 'add');

  if (setup) {
    return (
      <>
        <PageHeader title="Set up rooms" description="Room classes first, then the rooms in each class." />
        <div className="max-w-3xl">
          <RoomSetup
            hostelId={desk.hostelId}
            currency={desk.currency}
            steps={['Room classes', 'Rooms']}
            startOnRooms={(classes.data?.length ?? 0) > 0}
            doneLabel="Done"
            onDone={() => {
              desk.reload();
              desk.navigate({ page: 'rooms' });
            }}
          />
        </div>
      </>
    );
  }

  const openSetup = () => desk.navigate({ page: 'rooms', tab: 'setup' });
  const add = desk.isAdmin && (
    <Button icon="plus" onClick={openSetup}>
      Add rooms
    </Button>
  );

  if (!rooms) {
    return (
      <>
        <PageHeader title="Rooms" description="Rooms, classes and prices." actions={add} />
        {loading ? (
          <Card>
            <Skeleton rows={8} />
          </Card>
        ) : (
          <Alert title="Could not load rooms" onRetry={reload}>
            {error}
          </Alert>
        )}
      </>
    );
  }

  const vacant = rooms.filter((r) => roomState(r) === 'vacant');
  const occupied = rooms.filter((r) => roomState(r) === 'occupied');
  const types = ROOM_TYPES.filter((t) => rooms.some((r) => r.room_type === t.value));
  const q = query.trim().toLowerCase();
  const shown = (filter === 'vacant' ? vacant : filter === 'occupied' ? occupied : rooms).filter(
    (r) => (type === '' || r.room_type === type) && r.room_number.toLowerCase().includes(q),
  );
  const clearFilters = () => {
    setQuery('');
    setType('');
  };
  const selected = rooms.find((r) => r.id === selectedId);

  const room: Column<Room> = {
    header: 'Room',
    cell: (r) => (
      <>
        <p className="font-medium text-gray-900">{r.room_number}</p>
        <p className="text-sm font-normal text-gray-500">{classLabel(r)}</p>
      </>
    ),
  };
  const residents: Column<Room> = {
    header: 'Residents',
    cell: (r) => <span className="tabular-nums">{r.residents}</span>,
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
  // With the DetailPanel open the table is narrower; the recess price is in the panel.
  const columns = selected
    ? [room, residents, status, semester]
    : [room, residents, status, semester, recess];

  return (
    <>
      <PageHeader
        title="Rooms"
        description={
          rooms.length === 0
            ? 'Rooms, classes and prices.'
            : `${rooms.length} ${rooms.length === 1 ? 'room' : 'rooms'} · ${occupied.length} occupied · ${vacant.length} vacant`
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
                ? 'Set up your room classes and rooms to start checking residents in.'
                : 'An administrator adds the rooms. They will show up here.'
            }
            action={
              desk.isAdmin && (
                <Button icon="plus" onClick={openSetup}>
                  Set up rooms
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
          <div className="flex flex-wrap items-center justify-between gap-3">
            <Tabs
              label="Room filter"
              value={filter}
              onChange={setFilter}
              items={[
                { value: 'all', label: 'All', count: rooms.length },
                { value: 'vacant', label: 'Vacant', count: vacant.length },
                { value: 'occupied', label: 'Occupied', count: occupied.length },
              ]}
            />
            <div className="flex min-w-72 flex-1 gap-3 sm:max-w-md">
              <SearchInput
                aria-label="Search rooms"
                placeholder="Search room number"
                className="flex-1"
                value={query}
                onChange={(e) => setQuery(e.target.value)}
              />
              {types.length > 1 && (
                <div className="w-36 shrink-0">
                  <Select aria-label="Room type" value={type} onChange={(e) => setType(e.target.value)}>
                    <option value="">All types</option>
                    {types.map((t) => (
                      <option key={t.value} value={t.value}>
                        {t.label}
                      </option>
                    ))}
                  </Select>
                </div>
              )}
            </div>
          </div>
          <Card flush>
            {shown.length === 0 && (q || type) ? (
              <EmptyState
                icon="magnifying-glass"
                title={q ? `No rooms match “${query.trim()}”` : 'No rooms of this type'}
                description={q ? 'Search by room number, like A01, or its prefix, like A.' : 'Try another tab or type.'}
                action={
                  <Button variant="secondary" onClick={clearFilters}>
                    Clear filters
                  </Button>
                }
              />
            ) : shown.length === 0 && filter !== 'all' ? (
              <EmptyState icon="home" title={NONE[filter].title} description={NONE[filter].description} />
            ) : (
              <Table
                key={`${filter}:${type}:${q}`}
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
      {classes.data && classes.data.length > 0 && (
        <Card
          title="Room classes"
          description="Each room takes its type and prices from its class. A new price applies to new check-ins."
        >
          <ClassList
            classes={classes.data}
            currency={desk.currency}
            action={(c) =>
              desk.isAdmin && (
                <Button variant="secondary" size="sm" icon="pencil-square" onClick={() => setEditing(c)}>
                  Edit
                </Button>
              )
            }
          />
        </Card>
      )}
      {selected && <RoomDetail room={selected} onClose={() => setSelectedId(null)} />}
      <Modal
        open={!!editing}
        onClose={() => setEditing(null)}
        title={editing ? `Edit ${classLabel(editing)}` : 'Edit class'}
        description="Its rooms follow. Residents already checked in keep the price they agreed."
        size="lg"
      >
        {editing && (
          <ClassForm
            key={editing.id}
            hostelId={desk.hostelId}
            currency={desk.currency}
            editing={editing}
            submitLabel="Save class"
            onCancel={() => setEditing(null)}
            onSaved={() => {
              setEditing(null);
              desk.toast({ tone: 'success', title: 'Room class saved.' });
              desk.reload();
            }}
          />
        )}
      </Modal>
    </>
  );
}

/** The selected room: its class, prices and who is staying there (from `active_allocations`). */
function RoomDetail({ room, onClose }: { room: Room; onClose: () => void }) {
  const desk = useDesk();
  const stays = useLoad(() => api.activeAllocations({ hostel_id: desk.hostelId }), [desk.hostelId]);
  const state = roomState(room);
  const badge = STATE_BADGE[state];
  const here = stays.data?.filter((s) => s.room_number === room.room_number) ?? [];

  return (
    <DetailPanel
      title={`Room ${room.room_number}`}
      subtitle={classLabel(room)}
      onClose={onClose}
      footer={
        <Button
          className="flex-1"
          icon="user-plus"
          disabled={state === 'maintenance'}
          onClick={() => desk.navigate({ page: 'residents', tab: 'check-in' })}
        >
          Check in a resident
        </Button>
      }
    >
      <div className="flex items-center gap-2">
        <Badge tone={badge.tone}>{badge.label}</Badge>
        {state === 'maintenance' && <span className="text-sm text-gray-500">Not taking check-ins</span>}
      </div>
      <MiniStats items={[{ label: 'Residents', value: room.residents }]} />
      <DetailSection label="Prices">
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
                  className="flex w-full items-center gap-3 rounded-full px-2 py-2 text-start transition-colors hover:bg-gray-50 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-primary-900"
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
