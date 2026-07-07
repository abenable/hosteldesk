import { useEffect, useState } from 'react';
import { Button } from '../components/ui/Button.tsx';
import { Tabs } from '../components/ui/Tabs.tsx';
import { Modal } from '../components/ui/Modal.tsx';
import { RoomStatusBoard } from '../components/RoomStatusBoard.tsx';
import { RoomForm } from '../components/RoomForm.tsx';
import { PlusIcon } from '../components/ui/icons.tsx';
import { getRooms, listHostels, type Room, type Hostel } from '../lib/tauri.ts';

type Filter = 'all' | 'vacant' | 'occupied' | 'maintenance';

export function Rooms() {
  const [filter, setFilter] = useState<Filter>('all');
  const [addOpen, setAddOpen] = useState(false);
  const [rooms, setRooms] = useState<Room[]>([]);
  const [hostel, setHostel] = useState<Hostel | null>(null);
  const [refreshKey, setRefreshKey] = useState(0);

  useEffect(() => {
    let cancelled = false;
    Promise.all([getRooms(), listHostels()])
      .catch(() => [null, null] as const)
      .then(([r, h]) => {
        if (cancelled) return;
        if (Array.isArray(r)) setRooms(r);
        if (Array.isArray(h)) setHostel(h[0] ?? null);
      });
    return () => {
      cancelled = true;
    };
  }, [refreshKey]);

  const tabs = [
    { value: 'all', label: 'All', content: null },
    { value: 'vacant', label: 'Vacant', content: null },
    { value: 'occupied', label: 'Occupied', content: null },
    { value: 'maintenance', label: 'Maintenance', content: null },
  ];

  return (
    <div className="flex flex-col gap-6">
      <div className="flex items-center justify-between gap-4">
        <Tabs
          tabs={tabs}
          value={filter}
          onValueChange={(v) => setFilter(v as Filter)}
        />
        <Button onClick={() => setAddOpen(true)}>
          <PlusIcon width={16} height={16} />
          Add room
        </Button>
      </div>

      <RoomStatusBoard filter={filter} refreshKey={refreshKey} />

      <Modal
        open={addOpen}
        onOpenChange={setAddOpen}
        title="Add room"
        description="Floors are lettered A-first; room numbers follow A01, A02, B01…"
      >
        {hostel ? (
          <RoomForm
            hostelId={hostel.id}
            existing={rooms}
            onCreated={() => {
              setAddOpen(false);
              setRefreshKey((k) => k + 1);
            }}
          />
        ) : (
          <p className="text-body-sm text-muted">
            No hostel found. Complete onboarding first.
          </p>
        )}
      </Modal>
    </div>
  );
}
