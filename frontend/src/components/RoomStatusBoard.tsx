import { useEffect, useState } from 'react';
import { getRooms, type Room } from '../lib/tauri.ts';
import { Card } from './ui/Card.tsx';

export function RoomStatusBoard() {
  const [rooms, setRooms] = useState<Room[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    setError(null);
    getRooms()
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
  }, []);

  if (loading) {
    return <p className="text-body-sm text-muted">Loading rooms…</p>;
  }

  if (error) {
    return (
      <Card className="p-6 text-center text-body-sm text-error-text">
        Could not load rooms: {error}
      </Card>
    );
  }

  if (rooms.length === 0) {
    return (
      <Card className="p-6 text-center text-body-sm text-muted">
        No rooms found. Add rooms to see the status board.
      </Card>
    );
  }

  return (
    <section>
      <div className="mb-4 flex gap-4 text-caption text-muted">
        <span className="flex items-center gap-2">
          <span className="h-2.5 w-2.5 rounded-full bg-surface-strong" />
          Vacant
        </span>
        <span className="flex items-center gap-2">
          <span className="h-2.5 w-2.5 rounded-full bg-ink" />
          Occupied
        </span>
        <span className="flex items-center gap-2">
          <span className="h-2.5 w-2.5 rounded-full bg-primary" />
          Maintenance
        </span>
      </div>

      <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4">
        {rooms.map((room) => {
          const status = room.status.toLowerCase();
          const bg =
            status === 'vacant'
              ? 'bg-surface-soft'
              : status === 'occupied'
                ? 'bg-ink'
                : 'bg-muted-soft';
          const onDark = status === 'occupied';
          return (
            <Card key={room.id} className={`p-4 ${bg}`}>
              <div className="flex items-start justify-between">
                <h3
                  className={`text-title-md ${
                    onDark ? 'text-on-primary' : 'text-ink'
                  }`}
                >
                  {room.room_number}
                </h3>
                <span
                  className={`flex items-center gap-1.5 text-caption ${
                    onDark ? 'text-on-primary' : 'text-muted'
                  }`}
                >
                  {status === 'maintenance' && (
                    <span className="h-2 w-2 rounded-full bg-primary" />
                  )}
                  <span className="capitalize">{room.status}</span>
                </span>
              </div>
              <p
                className={`mt-1 text-body-sm capitalize ${
                  onDark ? 'text-on-primary opacity-80' : 'text-muted'
                }`}
              >
                {room.room_type} · Capacity {room.capacity}
              </p>
              {room.rate_per_period && (
                <p
                  className={`mt-2 text-body-sm ${
                    onDark ? 'text-on-primary' : 'text-ink'
                  }`}
                >
                  {room.rate_per_period} / period
                </p>
              )}
            </Card>
          );
        })}
      </div>
    </section>
  );
}
