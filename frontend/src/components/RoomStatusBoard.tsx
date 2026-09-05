import { useEffect, useState } from 'react';
import { getRooms, listHostels, type Room } from '../lib/tauri.ts';
import { Card } from './ui/Card.tsx';
import { groupByFloor } from '../lib/roomNumber.ts';
import { useHostel } from '../hooks/use-hostel.ts';

type Props = {
  filter?: 'all' | 'vacant' | 'occupied' | 'maintenance';
  refreshKey?: number;
  hostelId?: number;
};

export function RoomStatusBoard({ filter = 'all', refreshKey = 0, hostelId }: Props) {
  const { currency } = useHostel();
  const [rooms, setRooms] = useState<Room[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    setError(null);
    (hostelId !== undefined
      ? Promise.resolve(hostelId)
      : listHostels().then((h) => h[0]?.id ?? 0)
    )
      .then((hid) => getRooms(hid))
      .catch((e) => {
        setError(e instanceof Error ? e.message : String(e));
        return [] as Room[];
      })
      .then((data) => {
        if (!cancelled) setRooms(data);
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [refreshKey, hostelId]);

  if (loading) return <p className="text-body-sm text-muted">Loading rooms…</p>;

  if (error)
    return (
      <Card className="p-6 text-center text-body-sm text-error-text">
        Could not load rooms: {error}
      </Card>
    );

  const filtered =
    filter === 'all'
      ? rooms
      : rooms.filter((r) => r.status.toLowerCase() === filter);

  if (filtered.length === 0)
    return (
      <Card className="p-6 text-center text-body-sm text-muted">
        No {filter === 'all' ? '' : filter + ' '}rooms found.
      </Card>
    );

  const floors = groupByFloor(filtered);

  return (
    <section className="flex flex-col gap-6">
      {floors.map(({ floor, rooms: floorRooms }) => (
        <div key={floor}>
          <h3 className="mb-3 text-uppercase-tag text-muted">Floor {floor}</h3>
          <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4">
            {floorRooms.map((room) => {
              const status = room.status.toLowerCase();
              const isVacant = status === 'vacant';
              const isOccupied = status === 'occupied';
              return (
                <Card
                  key={room.id}
                  className={`p-4 ${isVacant ? 'bg-surface-soft' : isOccupied ? 'bg-ink' : 'bg-canvas border border-primary'}`}
                >
                  <div className="flex items-start justify-between">
                    <h4
                      className={`text-title-md ${isOccupied ? 'text-on-primary' : 'text-ink'}`}
                    >
                      {room.room_number}
                    </h4>
                    <span
                      className={`flex items-center gap-1.5 text-caption ${
                        isOccupied ? 'text-on-primary' : 'text-muted'
                      }`}
                    >
                      <span
                        className={`h-2 w-2 rounded-full ${
                          isVacant ? 'bg-muted-soft' : isOccupied ? 'bg-on-primary' : 'bg-primary'
                        }`}
                      />
                      <span className="capitalize">{room.status}</span>
                    </span>
                  </div>
                  <p
                    className={`mt-1 text-body-sm capitalize ${
                      isOccupied ? 'text-on-primary opacity-80' : 'text-muted'
                    }`}
                  >
                    {room.room_type} · Capacity {room.capacity}
                  </p>
                  <p
                    className={`mt-2 text-body-sm font-medium ${
                      isOccupied ? 'text-on-primary' : 'text-ink'
                    }`}
                  >
                    {currency} {room.semester_rate} <span className="font-normal">semester</span>
                  </p>
                  <p
                    className={`text-caption-sm ${
                      isOccupied ? 'text-on-primary opacity-80' : 'text-muted'
                    }`}
                  >
                    {currency} {room.recess_rate} <span>recess</span>
                  </p>
                </Card>
              );
            })}
          </div>
        </div>
      ))}
    </section>
  );
}
