import { useEffect, useState, useCallback } from 'react';
import { Card } from '../components/ui/Card.tsx';
import { Button } from '../components/ui/Button.tsx';
import { Badge } from '../components/ui/Badge.tsx';
import { Modal } from '../components/ui/Modal.tsx';
import { Table, Td, Tr } from '../components/ui/Table.tsx';
import { CheckInForm } from '../components/CheckInForm.tsx';
import { CheckOutConfirm } from '../components/CheckOutConfirm.tsx';
import { CurrencyAmount } from '../components/ui/CurrencyAmount.tsx';
import { useHostel } from '../hooks/use-hostel.ts';
import { formatDecimal } from '../lib/decimal.ts';
import {
  getDashboardStats,
  listRecentActivity,
  listActiveAllocations,
  type DashboardStats,
  type ActivityItem,
  type ActiveAllocation,
} from '../lib/tauri.ts';
import type { View } from '../components/types';

type Props = { onNavigate: (v: View) => void };

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

export function Dashboard({ onNavigate }: Props) {
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

  const departures = allocations.filter((a) => a.expected_checkout <= today);
  const pct = stats && stats.total_rooms > 0
    ? Math.round((stats.occupied / stats.total_rooms) * 100)
    : 0;
  const outstanding = stats?.outstanding_total ?? '0';
  const owed = BigInt(outstanding.replace('.', '').replace(/[^0-9]/g, '') || '0') > 0n;

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
          Check in
        </Button>
      </div>

      <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
        <SectionCard
          label="Occupancy"
          value={loading ? '—' : `${pct}%`}
          hint={stats ? `${stats.occupied} of ${stats.total_rooms} rooms` : undefined}
          trend={stats ? `${stats.check_ins_today} arrivals today` : undefined}
        />
        <SectionCard
          label="Available"
          value={loading ? '—' : (stats?.vacant ?? 0)}
          hint="Vacant and ready"
        />
        <SectionCard
          label="Revenue Today"
          value={loading ? '—' : `${currency} ${formatDecimal(stats?.revenue_today ?? '0')}`}
          hint={stats ? `${currency} ${formatDecimal(stats.revenue_month)} this month` : undefined}
        />
        <SectionCard
          label="Outstanding"
          value={loading ? '—' : `${currency} ${formatDecimal(outstanding)}`}
          hint={owed ? 'Collect payments' : 'All paid up'}
          error={owed}
        />
      </div>

      <div className="grid gap-4 lg:grid-cols-2">
        <Card shadow className="p-5">
          <div className="mb-4 flex items-center justify-between">
            <h2 className="text-title-sm text-ink">Departures</h2>
            <span className="text-caption text-muted">{departures.length} today</span>
          </div>
          {departures.length === 0 ? (
            <p className="text-body-sm text-muted">No departures today</p>
          ) : (
            <Table>
              <tbody>
                {departures.map((a) => {
                  const overdue = a.expected_checkout < today;
                  return (
                    <Tr key={a.allocation_id}>
                      <Td>
                        <div className="flex items-center gap-2">
                          <span className="text-body-sm text-ink">{a.resident_name}</span>
                          {overdue && <Badge className="bg-error-text text-on-primary">Overdue</Badge>}
                        </div>
                        <span className="text-caption-sm text-muted">{a.room_number}</span>
                      </Td>
                      <Td className="text-right">
                        <CurrencyAmount amount={a.balance} currency={currency} error={BigInt(a.balance.replace('.', '').replace(/[^0-9]/g, '') || '0') > 0n} muted />
                      </Td>
                      <Td className="text-right">
                        <Button variant="tertiary" size="sm" onClick={() => setCheckoutTarget(a)}>
                          Check out
                        </Button>
                      </Td>
                    </Tr>
                  );
                })}
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

      <Card shadow className="p-5">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-6">
            {[
              { label: 'Vacant', value: stats?.vacant ?? 0, dot: 'bg-surface-strong' },
              { label: 'Occupied', value: stats?.occupied ?? 0, dot: 'bg-ink' },
              { label: 'Maintenance', value: stats?.maintenance ?? 0, dot: 'bg-primary' },
            ].map((s) => (
              <span key={s.label} className="flex items-center gap-2 text-caption text-muted">
                <span className={`h-2.5 w-2.5 rounded-full ${s.dot}`} />
                {s.label} {s.value}
              </span>
            ))}
          </div>
          <Button variant="tertiary" size="sm" onClick={() => onNavigate('rooms')}>
            View rooms →
          </Button>
        </div>
      </Card>

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

function SectionCard({
  label,
  value,
  hint,
  trend,
  error,
}: {
  label: string;
  value: React.ReactNode;
  hint?: string;
  trend?: string;
  error?: boolean;
}) {
  return (
    <Card shadow className="p-5">
      <div className="flex items-center justify-between">
        <p className="text-caption text-muted">{label}</p>
        {trend && (
          <Badge className="bg-surface-soft text-muted">{trend}</Badge>
        )}
      </div>
      <p className={`mt-2 text-display-md ${error ? 'text-error-text' : 'text-ink'}`}>{value}</p>
      {hint && <p className="mt-1 text-caption-sm text-muted">{hint}</p>}
    </Card>
  );
}
