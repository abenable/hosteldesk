import { useState } from 'react';
import { CheckInForm } from '../components/CheckInForm.tsx';
import { RoomStatusBoard } from '../components/RoomStatusBoard.tsx';

export function Dashboard() {
  const [refreshKey, setRefreshKey] = useState(0);

  return (
    <div className="grid gap-6 lg:grid-cols-3">
      <div className="lg:col-span-2">
        <h2 className="mb-3 text-title-md text-ink">Room status</h2>
        <RoomStatusBoard key={refreshKey} />
      </div>
      <div>
        <h2 className="mb-3 text-title-md text-ink">Check in</h2>
        <CheckInForm onSuccess={() => setRefreshKey((k) => k + 1)} />
      </div>
    </div>
  );
}
