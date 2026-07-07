import { useEffect, useMemo, useState } from 'react';
import { Card } from '../components/ui/Card.tsx';
import { Button } from '../components/ui/Button.tsx';
import { StatCard } from '../components/ui/StatCard.tsx';
import { EmptyState } from '../components/ui/EmptyState.tsx';
import { Modal } from '../components/ui/Modal.tsx';
import { CheckInForm } from '../components/CheckInForm.tsx';
import { getRooms, type Room } from '../lib/tauri.ts';
import { PlusIcon, RoomsIcon, InvoiceIcon, ResidentsIcon } from '../components/ui/icons.tsx';

type QuickAction = {
  label: string;
  icon: typeof PlusIcon;
  onClick: () => void;
  disabled?: boolean;
};

function occupancy(rooms: Room[]) {
  const total = rooms.length;
  const occupied = rooms.filter((r) => r.status.toLowerCase() === 'occupied').length;
  const vacant = rooms.filter((r) => r.status.toLowerCase() === 'vacant').length;
  const maintenance = rooms.filter((r) => r.status.toLowerCase() === 'maintenance').length;
  const pct = total === 0 ? 0 : Math.round((occupied / total) * 100);
  return { total, occupied, vacant, maintenance, pct };
}

export function Dashboard() {
  const [rooms, setRooms] = useState<Room[]>([]);
  const [loading, setLoading] = useState(true);
  const [checkInOpen, setCheckInOpen] = useState(false);
  const [refreshKey, setRefreshKey] = useState(0);

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    getRooms()
      .catch(() => [] as Room[])
      .then((data) => {
        if (!cancelled) setRooms(data);
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [refreshKey]);

  const stats = useMemo(() => occupancy(rooms), [rooms]);

  const quickActions: QuickAction[] = [
    { label: 'Check in', icon: PlusIcon, onClick: () => setCheckInOpen(true) },
    {
      label: 'Add room',
      icon: RoomsIcon,
      onClick: () => setRefreshKey((k) => k + 1),
      disabled: true,
    },
    { label: 'Record payment', icon: InvoiceIcon, onClick: () => undefined, disabled: true },
  ];

  return (
    <div className="flex flex-col gap-6">
      <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
        <StatCard
          label="OCCUPANCY"
          value={loading ? '—' : `${stats.pct}%`}
          hint={`${stats.occupied} of ${stats.total} rooms occupied`}
          icon={<RoomsIcon width={18} height={18} />}
        />
        <StatCard
          label="AVAILABLE"
          value={loading ? '—' : stats.vacant}
          hint="Vacant and ready"
        />
        <StatCard
          label="CHECK-INS TODAY"
          value="—"
          hint="Resident list command pending"
          icon={<ResidentsIcon width={18} height={18} />}
        />
        <StatCard
          label="REVENUE TODAY"
          value="—"
          hint="Payments list command pending"
          icon={<InvoiceIcon width={18} height={18} />}
        />
      </div>

      <div className="grid gap-6 lg:grid-cols-3">
        <Card shadow className="p-5 lg:col-span-2">
          <div className="mb-4 flex items-center justify-between">
            <h2 className="text-title-md text-ink">Room status</h2>
            <span className="text-caption text-muted">{stats.total} rooms</span>
          </div>
          <div className="grid grid-cols-3 gap-3">
            {[
              { label: 'Vacant', value: stats.vacant, dot: 'bg-surface-strong' },
              { label: 'Occupied', value: stats.occupied, dot: 'bg-ink' },
              { label: 'Maintenance', value: stats.maintenance, dot: 'bg-primary' },
            ].map((s) => (
              <div
                key={s.label}
                className="flex flex-col gap-1 rounded-md border border-hairline p-4"
              >
                <span className="flex items-center gap-2 text-caption text-muted">
                  <span className={`h-2.5 w-2.5 rounded-full ${s.dot}`} />
                  {s.label}
                </span>
                <span className="text-display-md text-ink">{s.value}</span>
              </div>
            ))}
          </div>
        </Card>

        <Card shadow className="p-5">
          <h2 className="mb-4 text-title-md text-ink">Recent activity</h2>
          <EmptyState
            title="Nothing to show yet"
            description="Recent check-ins and payments will appear here once the activity feed command lands."
          />
        </Card>
      </div>

      <Card shadow className="p-5">
        <h2 className="mb-4 text-title-md text-ink">Quick actions</h2>
        <div className="flex flex-wrap gap-3">
          {quickActions.map((a) => {
            const Icon = a.icon;
            return (
              <Button
                key={a.label}
                variant="secondary"
                onClick={a.onClick}
                disabled={a.disabled}
              >
                <Icon width={16} height={16} />
                {a.label}
              </Button>
            );
          })}
        </div>
      </Card>

      <Modal
        open={checkInOpen}
        onOpenChange={setCheckInOpen}
        title="Check in resident"
        description="Assign a vacant room to a new resident."
      >
        <CheckInForm
          onSuccess={() => {
            setCheckInOpen(false);
            setRefreshKey((k) => k + 1);
          }}
        />
      </Modal>
    </div>
  );
}
