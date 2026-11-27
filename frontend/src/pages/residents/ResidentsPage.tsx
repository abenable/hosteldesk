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
  Icon,
  Input,
  MiniStats,
  NameCell,
  PageHeader,
  Skeleton,
  Table,
  Tabs,
} from '../../components/ui';
import * as api from '../../lib/api';
import { useDesk } from '../../lib/desk';
import { methodLabel, termLabel } from '../../lib/format';
import { isOwing } from '../../lib/money';
import type { ActiveAllocation, ResidentSummary } from '../../lib/types';
import { useLoad } from '../../lib/useLoad';
import TakePaymentModal from '../payments/TakePaymentModal';
import CheckInModal from './CheckInModal';
import { useCheckOut } from './stay';

type Tab = 'current' | 'past' | 'all';
type Returning = { id: number; full_name: string } | null;

/** Paid (green) / Owing (red) by balance; "No stay" for someone never checked in. */
function BalanceBadge({ balance, never }: { balance: string; never?: boolean }) {
  if (never) return <Badge>No stay</Badge>;
  return isOwing(balance) ? <Badge tone="danger">Owing</Badge> : <Badge tone="success">Paid</Badge>;
}

const matches = (r: ResidentSummary, query: string) => {
  const digits = query.replace(/\s/g, '');
  return (
    r.full_name.toLowerCase().includes(query) ||
    (digits !== '' && (r.phone ?? '').replace(/\s/g, '').includes(digits)) ||
    (r.room_number ?? '').toLowerCase().includes(query)
  );
};

/**
 * The resident directory (ui.rs `residents`): Current / Past / All tabs,
 * search by name, phone or room, a DetailPanel for the selected resident,
 * check-in (also opened by `route.tab === 'check-in'`) and take payment.
 */
export default function ResidentsPage() {
  const desk = useDesk();
  const list = useLoad(() => api.listResidents({ hostel_id: desk.hostelId }), [desk.hostelId]);
  const stays = useLoad(() => api.activeAllocations({ hostel_id: desk.hostelId }), [desk.hostelId]);
  const [tab, setTab] = useState<Tab>('current');
  const [query, setQuery] = useState('');
  const [selected, setSelected] = useState<number | null>(null);
  const [payingId, setPayingId] = useState<number | null>(null);
  const paying = stays.data?.find((s) => s.allocation_id === payingId) ?? null;
  // Keyed by `key`: every opening starts with an empty form. The dashboard links here with tab "check-in".
  const [checkIn, setCheckIn] = useState(() => ({
    key: 0,
    open: desk.route.tab === 'check-in',
    returning: null as Returning,
  }));
  const openCheckIn = (returning: Returning = null) =>
    setCheckIn((c) => ({ key: c.key + 1, open: true, returning }));

  const residents = list.data ?? [];
  const current = residents.filter((r) => r.status === 'active');
  const past = residents.filter((r) => r.status === 'checked_out');
  const owing = residents.filter((r) => isOwing(r.balance)).length;
  const q = query.trim().toLowerCase();
  const rows = (tab === 'current' ? current : tab === 'past' ? past : residents).filter(
    (r) => q === '' || matches(r, q),
  );
  const chosen = residents.find((r) => r.id === selected);

  const checkInButton = (
    <Button icon="user-plus" onClick={() => openCheckIn()}>
      Check in
    </Button>
  );

  let body;
  if (list.loading) {
    body = (
      <Card>
        <Skeleton rows={8} />
      </Card>
    );
  } else if (list.error && !list.data) {
    body = (
      <Alert title="Could not load residents" onRetry={list.reload}>
        {list.error}
      </Alert>
    );
  } else if (residents.length === 0) {
    body = (
      <Card>
        <EmptyState
          icon="user-group"
          title="No residents yet"
          description="Check in your first resident to start tracking their stay and payments."
          action={checkInButton}
        />
      </Card>
    );
  } else {
    body = (
      <>
        <div className="flex flex-wrap items-center justify-between gap-3">
          <Tabs
            label="Residents"
            value={tab}
            onChange={setTab}
            items={[
              { value: 'current', label: 'Current', count: current.length },
              { value: 'past', label: 'Past', count: past.length },
              { value: 'all', label: 'All', count: residents.length },
            ]}
          />
          <div className="relative min-w-56 flex-1 sm:max-w-xs">
            <Icon
              name="magnifying-glass"
              className="pointer-events-none absolute inset-y-0 start-4 my-auto size-4 text-gray-400"
            />
            <Input
              type="search"
              aria-label="Search residents"
              placeholder="Search name, phone or room"
              className="ps-10 [&::-webkit-search-cancel-button]:hidden"
              value={query}
              onChange={(e) => setQuery(e.target.value)}
            />
          </div>
        </div>
        <Card flush>
          {rows.length > 0 ? (
            <Table
              key={`${tab}:${q}`}
              caption="Residents"
              rows={rows}
              rowKey={(r) => r.id}
              selectedKey={selected}
              onRowClick={(r) => setSelected(r.id)}
              pageSize={10}
              noun="residents"
              columns={[
                {
                  header: 'Resident',
                  cell: (r) => (
                    <NameCell
                      name={r.full_name}
                      detail={
                        [r.status === 'active' ? `Room ${r.room_number ?? '—'}` : null, r.phone ?? r.email]
                          .filter(Boolean)
                          .join(' · ') || '—'
                      }
                    />
                  ),
                },
                {
                  header: 'Status',
                  cell: (r) => <BalanceBadge balance={r.balance} never={r.status === 'never_allocated'} />,
                },
                {
                  header: 'Balance',
                  align: 'end',
                  cell: (r) => (
                    <span className={isOwing(r.balance) ? 'font-medium text-gray-900' : 'text-gray-500'}>
                      {desk.formatMoney(r.balance)}
                    </span>
                  ),
                },
              ]}
            />
          ) : q ? (
            <EmptyState
              icon="magnifying-glass"
              title={`No residents match “${query.trim()}”`}
              description="Search by name, phone number or room number."
              action={
                <Button variant="secondary" onClick={() => setQuery('')}>
                  Clear search
                </Button>
              }
            />
          ) : tab === 'current' ? (
            <EmptyState
              icon="user-group"
              title="No one is checked in"
              description="Residents with an active stay show up here."
              action={checkInButton}
            />
          ) : (
            <EmptyState
              icon="user-group"
              title="No past residents"
              description="Residents show up here after they check out."
            />
          )}
        </Card>
      </>
    );
  }

  return (
    <>
      <PageHeader
        title="Residents"
        description={
          residents.length > 0
            ? `${current.length} current · ${owing} owing`
            : 'Who is staying, who has left, and what they owe.'
        }
        actions={checkInButton}
      />
      {body}
      {chosen && (
        <ResidentPanel
          key={chosen.id}
          summary={chosen}
          stay={stays.data?.find((s) => s.allocation_id === chosen.allocation_id) ?? null}
          staysLoading={stays.loading}
          onClose={() => setSelected(null)}
          onPay={(s) => setPayingId(s.allocation_id)}
          onCheckInAgain={() => openCheckIn({ id: chosen.id, full_name: chosen.full_name })}
        />
      )}
      <CheckInModal
        key={checkIn.key}
        open={checkIn.open}
        returning={checkIn.returning}
        onClose={() => setCheckIn((c) => ({ ...c, open: false }))}
      />
      <TakePaymentModal stay={paying} onClose={() => setPayingId(null)} />
    </>
  );
}

/** One label/value line of the panel's contact list. */
function Line({ label, value }: { label: string; value: string | null }) {
  return (
    <div className="flex justify-between gap-4">
      <dt className="shrink-0 text-gray-500">{label}</dt>
      <dd className="min-w-0 truncate text-end">{value || '—'}</dd>
    </div>
  );
}

const PANEL_PAYMENTS = 5;

/** The selected resident (ui.rs `resident_detail`), docked on the right like Tabula's guest panel. */
function ResidentPanel({
  summary,
  stay,
  staysLoading,
  onClose,
  onPay,
  onCheckInAgain,
}: {
  summary: ResidentSummary;
  /** The active stay with what it has paid and owes, when there is one. */
  stay: ActiveAllocation | null;
  staysLoading: boolean;
  onClose: () => void;
  onPay: (stay: ActiveAllocation) => void;
  onCheckInAgain: () => void;
}) {
  const desk = useDesk();
  const checkOut = useCheckOut();
  const detail = useLoad(
    () => api.residentDetail({ hostel_id: desk.hostelId, resident_id: summary.id }),
    [desk.hostelId, summary.id],
  );
  const d = detail.data;
  const active = d?.allocations.find((a) => a.status === 'active');
  const isActive = summary.status === 'active';

  let body;
  if (detail.loading) {
    body = <Skeleton rows={8} />;
  } else if (!d) {
    body = (
      <Alert title="Could not load this resident" onRetry={detail.reload}>
        {detail.error}
      </Alert>
    );
  } else {
    const r = d.resident;
    body = (
      <>
        <div className="flex items-center gap-4">
          <Avatar name={r.full_name} size="lg" />
          <div className="min-w-0 space-y-1.5">
            <p className="truncate text-sm text-gray-700">{r.phone ?? r.email ?? 'No phone number'}</p>
            <div className="flex flex-wrap gap-1.5">
              <BalanceBadge balance={d.balance} never={d.allocations.length === 0} />
              {d.allocations.length > 0 && <Badge>{active ? 'Checked in' : 'Checked out'}</Badge>}
            </div>
          </div>
        </div>

        {active && (
          <Alert
            tone="info"
            icon="calendar-days"
            title={`Room ${active.room_number} · ${termLabel(active.term)} · ${active.academic_year}`}
            action={
              <Button
                size="sm"
                variant="secondary"
                onClick={() =>
                  void checkOut({
                    allocation_id: active.id,
                    name: r.full_name,
                    balance: stay?.balance ?? d.balance,
                  })
                }
              >
                Check out
              </Button>
            }
          >
            Rate agreed {desk.formatMoney(active.rate_agreed)} · since {desk.formatDate(active.created_at)}
          </Alert>
        )}

        {/* Two tiles, so money never wraps in the 320 px panel; the stay count is on the Stays label. */}
        <MiniStats
          items={[
            { label: 'Balance', value: desk.formatMoney(d.balance) },
            stay
              ? { label: 'Paid this stay', value: desk.formatMoney(stay.total_paid) }
              : { label: 'Payments', value: d.payments.length },
          ]}
        />

        <DetailSection label="Contact">
          <dl className="space-y-2">
            <Line label="Email" value={r.email} />
            <Line label="ID number" value={r.id_number} />
          </dl>
        </DetailSection>

        <DetailSection label={`Stays · ${d.allocations.length}`}>
          {d.allocations.length === 0 ? (
            <p className="text-gray-500">Never checked in.</p>
          ) : (
            <ul className="divide-y divide-gray-200">
              {d.allocations.map((a) => (
                <li key={a.id} className="flex items-start justify-between gap-4 py-2.5 first:pt-0 last:pb-0">
                  <div className="min-w-0">
                    <p className="font-medium">
                      Room {a.room_number} · {termLabel(a.term)}
                    </p>
                    <p className="text-gray-500">
                      {a.academic_year} · {a.status === 'active' ? 'current' : 'checked out'}
                    </p>
                  </div>
                  <p className="shrink-0 tabular-nums">{desk.formatMoney(a.rate_agreed)}</p>
                </li>
              ))}
            </ul>
          )}
        </DetailSection>

        <DetailSection label="Recent payments">
          {d.payments.length === 0 ? (
            <p className="text-gray-500">No payments yet.</p>
          ) : (
            <ul className="divide-y divide-gray-200">
              {d.payments.slice(0, PANEL_PAYMENTS).map((p) => (
                <li key={p.id} className="flex items-start justify-between gap-4 py-2.5 first:pt-0 last:pb-0">
                  <div className="min-w-0">
                    <p className="font-medium">{desk.formatDate(p.payment_date)}</p>
                    <p className="truncate text-gray-500">
                      {[methodLabel(p.payment_method), p.notes].filter(Boolean).join(' · ')}
                    </p>
                  </div>
                  <p className="shrink-0 font-medium tabular-nums">{desk.formatMoney(p.amount)}</p>
                </li>
              ))}
            </ul>
          )}
          {d.payments.length > PANEL_PAYMENTS && (
            <p className="mt-2 text-xs text-gray-500">
              The last {PANEL_PAYMENTS} of {d.payments.length}. The full profile lists them all.
            </p>
          )}
        </DetailSection>

        {isOwing(d.balance) && (
          <Alert tone="warning" title={`Owes ${desk.formatMoney(d.balance)}`}>
            {d.payments[0]
              ? `Last paid ${desk.formatDate(d.payments[0].payment_date)}.`
              : 'No payments yet.'}
          </Alert>
        )}
      </>
    );
  }

  return (
    <DetailPanel
      title={summary.full_name}
      subtitle={
        d
          ? `Resident since ${desk.formatDate(d.resident.created_at)}`
          : isActive
            ? `Room ${summary.room_number ?? '—'}`
            : 'Checked out'
      }
      onClose={onClose}
      footer={
        <>
          <Button
            variant="secondary"
            onClick={() => desk.navigate({ page: 'residents', residentId: summary.id })}
          >
            View full profile
          </Button>
          {isActive ? (
            <Button
              className="flex-1"
              loading={staysLoading}
              disabled={!stay}
              onClick={() => stay && onPay(stay)}
            >
              Take payment
            </Button>
          ) : (
            <Button className="flex-1" onClick={onCheckInAgain}>
              Check in again
            </Button>
          )}
        </>
      }
    >
      {body}
    </DetailPanel>
  );
}
