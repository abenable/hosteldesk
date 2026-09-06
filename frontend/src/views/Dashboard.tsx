import { useEffect, useState, useCallback } from 'react';
import { Card } from '../components/ui/Card.tsx';
import { Button } from '../components/ui/Button.tsx';
import { StatCard } from '../components/ui/StatCard.tsx';
import { Modal } from '../components/ui/Modal.tsx';
import { Table, Td, Tr } from '../components/ui/Table.tsx';
import { CheckInForm } from '../components/CheckInForm.tsx';
import { CheckOutConfirm } from '../components/CheckOutConfirm.tsx';
import { CurrencyAmount } from '../components/ui/CurrencyAmount.tsx';
import { useHostel } from '../hooks/use-hostel.ts';
import { formatDecimal, owesMoney } from '../lib/decimal.ts';
import {
  getDashboardStats,
  listRecentActivity,
  listActiveAllocations,
  type DashboardStats,
  type ActivityItem,
  type ActiveAllocation,
} from '../lib/tauri.ts';

function greeting(): string {
  const h = new Date().getHours();
  if (h < 12) return 'Good morning';
  if (h < 17) return 'Good afternoon';
  return 'Good evening';
}

function timeAgo(iso: string): string {
  const diff = Date.now() - new Date(iso).getTime();
  const hrs = diff / 3_600_000;
  if (hrs < 1) {
    const mins = Math.max(1, Math.round(diff / 60_000));
    return `${mins}m ago`;
  }
  if (hrs < 24) return `${Math.round(hrs)}h ago`;
  return new Date(iso).toLocaleDateString();
}

export function Dashboard() {
  const { hostel, hostelId, currency } = useHostel();
  const [stats, setStats] = useState<DashboardStats | null>(null);
  const [activity, setActivity] = useState<ActivityItem[]>([]);
  const [allocations, setAllocations] = useState<ActiveAllocation[]>([]);
  const [loading, setLoading] = useState(true);
  const [refreshKey, setRefreshKey] = useState(0);
  const [checkInOpen, setCheckInOpen] = useState(false);
  const [checkoutTarget, setCheckoutTarget] = useState<ActiveAllocation | null>(null);

  const today = new Date().toLocaleDateString('en-CA');
  const refresh = useCallback(() => setRefreshKey((k) => k + 1), []);

  useEffect(() => {
    if (!hostelId) return;
    let cancelled = false;
    setLoading(true);
    Promise.all([
      getDashboardStats({ hostel_id: hostelId, today }),
      listRecentActivity({ hostel_id: hostelId, limit: 8 }),
      listActiveAllocations({ hostel_id: hostelId }),
    ])
      .then(([s, a, al]) => {
        if (cancelled) return;
        setStats(s);
        setActivity(a);
        setAllocations(al);
      })
      .catch(() => {})
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => { cancelled = true; };
  }, [hostelId, today, refreshKey]);

  const pct = stats && stats.total_rooms > 0
    ? Math.round((stats.occupied / stats.total_rooms) * 100)
    : 0;
  const outstanding = stats?.outstanding_total ?? '0';
  const owed = owesMoney(outstanding);

  return (
    <div className="flex flex-col gap-4 md:gap-6">
      <div className="flex items-center justify-between gap-4">
        <div>
          <h1 className="text-display-sm text-ink">
            {greeting()}{hostel ? `, ${hostel.name}` : ''}
          </h1>
          <p className="text-caption-sm text-muted">
            {new Date().toLocaleDateString('en-US', { weekday: 'long', month: 'long', day: 'numeric' })}
          </p>
        </div>
        <Button onClick={() => setCheckInOpen(true)}>
          New Resident
        </Button>
      </div>

      <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
        <StatCard
          label="Occupancy"
          value={loading ? '—' : `${pct}%`}
          hint={stats ? `${stats.occupied} of ${stats.total_rooms} rooms` : undefined}
        />
        <StatCard
          label="Available"
          value={loading ? '—' : (stats?.vacant ?? 0)}
          hint="Vacant and ready"
        />
        <StatCard
          label="This month"
          value={loading ? '—' : `${currency} ${formatDecimal(stats?.revenue_month ?? '0')}`}
        />
        <StatCard
          label="Outstanding"
          value={loading ? '—' : `${currency} ${formatDecimal(outstanding)}`}
          hint={owed ? 'Collect payments' : 'All paid up'}
          error={owed}
        />
      </div>

      <div className="grid gap-4 lg:grid-cols-2">
        <Card shadow className="p-5">
          <div className="mb-4 flex items-center justify-between">
            <h2 className="text-title-sm text-ink">Active stays</h2>
            <span className="text-caption text-muted">{allocations.length}</span>
          </div>
          {allocations.length === 0 ? (
            <p className="text-body-sm text-muted">No active stays</p>
          ) : (
            <Table>
              <tbody>
                {allocations.map((a) => (
                  <Tr key={a.allocation_id}>
                    <Td>
                      <span className="text-body-sm text-ink">{a.resident_name}</span>
                      <span className="ml-2 text-caption-sm text-muted">{a.room_number}</span>
                    </Td>
                    <Td className="text-right">
                      <CurrencyAmount amount={a.balance} currency={currency} error={owesMoney(a.balance)} muted />
                    </Td>
                    <Td className="text-right">
                      <Button variant="tertiary" size="sm" onClick={() => setCheckoutTarget(a)}>
                        Check out
                      </Button>
                    </Td>
                  </Tr>
                ))}
              </tbody>
            </Table>
          )}
        </Card>

        <Card shadow className="p-5">
          <div className="mb-4 flex items-center justify-between">
            <h2 className="text-title-sm text-ink">Recent Activity</h2>
          </div>
          {activity.length === 0 ? (
            <p className="text-body-sm text-muted">
              Activity appears here as you check residents in and record payments.
            </p>
          ) : (
            <div className="flex flex-col gap-3">
              {activity.map((item, i) => (
                <div key={i} className="flex items-start gap-3">
                  <span className={`mt-0.5 h-2 w-2 shrink-0 rounded-full ${
                    item.kind === 'payment' ? 'bg-primary' :
                    item.kind === 'check_in' ? 'bg-ink' : 'bg-surface-strong'
                  }`} />
                  <div className="min-w-0 flex-1">
                    <p className="text-body-sm text-ink">
                      {item.kind === 'payment' && 'Payment'}
                      {item.kind === 'check_in' && 'Check-in'}
                      {item.kind === 'check_out' && 'Check-out'}
                      {' · '}
                      {item.resident_name}, {item.room_number}
                    </p>
                    <p className="text-caption-sm text-muted">
                      {item.amount && `${currency} ${formatDecimal(item.amount)} · `}
                      {timeAgo(item.at)}
                    </p>
                  </div>
                </div>
              ))}
            </div>
          )}
        </Card>
      </div>

      <Modal
        open={checkInOpen}
        onOpenChange={setCheckInOpen}
        title="Check in resident"
        description="Assign a vacant room to a new resident."
      >
        <CheckInForm onSuccess={() => { setCheckInOpen(false); refresh(); }} />
      </Modal>

      {checkoutTarget && (
        <CheckOutConfirm
          open={!!checkoutTarget}
          onOpenChange={(o) => !o && setCheckoutTarget(null)}
          hostelId={hostelId}
          currency={currency}
          allocationId={checkoutTarget.allocation_id}
          residentName={checkoutTarget.resident_name}
          roomNumber={checkoutTarget.room_number}
          balance={checkoutTarget.balance}
          onDone={refresh}
        />
      )}
    </div>
  );
}
