import { useEffect, useState } from 'react';
import { Button } from '../components/ui/Button.tsx';
import { Tabs } from '../components/ui/Tabs.tsx';
import { Modal } from '../components/ui/Modal.tsx';
import { RoomStatusBoard } from '../components/RoomStatusBoard.tsx';
import { RoomForm } from '../components/RoomForm.tsx';
import { useHostel } from '../hooks/use-hostel.ts';
import { getRooms, type Room } from '../lib/tauri.ts';

type Filter = 'all' | 'vacant' | 'occupied' | 'maintenance';

export function Rooms() {
  const { hostel, hostelId } = useHostel();
  const [filter, setFilter] = useState<Filter>('all');
  const [addOpen, setAddOpen] = useState(false);
  const [rooms, setRooms] = useState<Room[]>([]);
  const [refreshKey, setRefreshKey] = useState(0);

  useEffect(() => {
    if (!hostelId) return;
    let cancelled = false;
    setRooms([]);
    getRooms(hostelId)
      .then((r) => {
        if (!cancelled) setRooms(r);
      })
      .catch(() => {});
    return () => {
      cancelled = true;
    };
  }, [hostelId, refreshKey]);

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
          Add room
        </Button>
      </div>

      <RoomStatusBoard filter={filter} refreshKey={refreshKey} hostelId={hostel?.id} />

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
