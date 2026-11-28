import { useState, type ReactNode } from 'react';
import {
  Alert,
  Badge,
  Button,
  Card,
  DetailPanel,
  DetailSection,
  EmptyState,
  Field,
  Input,
  MiniStats,
  NameCell,
  PageHeader,
  Select,
  Skeleton,
  Table,
  Tabs,
  Toggle,
  type Column,
} from '../../components/ui';
import * as api from '../../lib/api';
import { useDesk } from '../../lib/desk';
import { METHODS, methodLabel, termLabel, todayIso } from '../../lib/format';
import { isOwing } from '../../lib/money';
import type { ActiveAllocation, PaymentRow } from '../../lib/types';
import { useLoad, type Load } from '../../lib/useLoad';
import TakePaymentModal from './TakePaymentModal';

type Tab = 'outstanding' | 'history';
type Filter = { from: string; to: string; method: string };
const NO_FILTER: Filter = { from: '', to: '', method: '' };

const stayLine = (s: ActiveAllocation) =>
  `Room ${s.room_number} · ${termLabel(s.term)} ${s.academic_year}`;

/** Red when nothing is paid on the stay, amber when part is, green when paid up (string tests only). */
function StayBadge({ stay }: { stay: ActiveAllocation }) {
  if (!isOwing(stay.balance)) return <Badge tone="success" icon="check-circle">Paid up</Badge>;
  return isOwing(stay.total_paid) ? (
    <Badge tone="warning">Partial</Badge>
  ) : (
    <Badge tone="danger">Owing</Badge>
  );
}

/** Outstanding balances of active stays, the payments ledger, taking payments and reopening receipts. */
export default function PaymentsPage() {
  const desk = useDesk();
  const { hostelId } = desk;
  const tab: Tab = desk.route.tab === 'history' ? 'history' : 'outstanding';
  const setTab = (next: Tab) => desk.navigate({ page: 'payments', tab: next });

  const stays = useLoad(() => api.activeAllocations({ hostel_id: hostelId }), [hostelId]);
  // Rust sums the outstanding total (Decimal); never add balances in TS.
  const stats = useLoad(() => api.dashboardStats({ hostel_id: hostelId, today: todayIso() }), [hostelId]);
  const [filter, setFilter] = useState<Filter>(NO_FILTER);
  const payments = useLoad(
    () =>
      api.listPayments({
        hostel_id: hostelId,
        from: filter.from || null,
        to: filter.to || null,
        resident_id: null,
        method: filter.method || null,
      }),
    [hostelId, filter],
  );
  const [payingId, setPayingId] = useState<number | null>(null);
  const paying = stays.data?.find((s) => s.allocation_id === payingId) ?? null;

  const owing = stays.data?.filter((s) => isOwing(s.balance));
  const description = owing
    ? [
        `${owing.length} of ${stays.data?.length ?? 0} active stays owing`,
        stats.data && `${desk.formatMoney(stats.data.outstanding_total)} outstanding`,
      ]
        .filter(Boolean)
        .join(' · ')
    : 'Take payments and print receipts.';
  const tabs = <PaymentTabs tab={tab} setTab={setTab} owing={owing?.length} paid={payments.data?.length} />;

  return (
    <>
      <PageHeader title="Payments" description={description} />
      {tab === 'outstanding' ? (
        <Outstanding
          tabs={tabs}
          stays={stays}
          onPay={(s) => setPayingId(s.allocation_id)}
        />
      ) : (
        <History
          tabs={tabs}
          payments={payments}
          filter={filter}
          setFilter={setFilter}
          onShowOutstanding={() => setTab('outstanding')}
        />
      )}
      <TakePaymentModal stay={paying} onClose={() => setPayingId(null)} />
    </>
  );
}

function PaymentTabs({
  tab,
  setTab,
  owing,
  paid,
}: {
  tab: Tab;
  setTab: (tab: Tab) => void;
  owing?: number;
  paid?: number;
}) {
  return (
    <Tabs
      label="Payments"
      value={tab}
      onChange={setTab}
      items={[
        { value: 'outstanding', label: 'Outstanding', count: owing },
        { value: 'history', label: 'History', count: paid },
      ]}
    />
  );
}

// ---------------------------------------------------------------- outstanding

function Outstanding({
  tabs,
  stays,
  onPay,
}: {
  tabs: ReactNode;
  stays: Load<ActiveAllocation[]>;
  onPay: (stay: ActiveAllocation) => void;
}) {
  const desk = useDesk();
  const [showAll, setShowAll] = useState(false);
  const [selectedId, setSelectedId] = useState<number | null>(null);
  const all = stays.data;
  const rows = all && (showAll ? all : all.filter((s) => isOwing(s.balance)));
  const selected = all?.find((s) => s.allocation_id === selectedId) ?? null;

  const resident: Column<ActiveAllocation> = {
    header: 'Resident',
    // The open panel's subtitle carries the term; keep the narrow table to the room.
    cell: (s) => <NameCell name={s.resident_name} detail={selected ? `Room ${s.room_number}` : stayLine(s)} />,
  };
  const paid: Column<ActiveAllocation> = {
    header: 'Paid',
    cell: (s) => desk.formatMoney(s.total_paid),
    align: 'end',
  };
  // The status badge sits under the balance: a separate column would push the row action off a 1280 px window.
  const balance: Column<ActiveAllocation> = {
    header: 'Balance',
    cell: (s) => (
      <div className="flex flex-col items-end gap-1">
        <span className="font-medium text-gray-900">{desk.formatMoney(s.balance)}</span>
        <StayBadge stay={s} />
      </div>
    ),
    align: 'end',
  };
  // With the panel open the table is narrow: the figures and the action move into the panel.
  const columns: Column<ActiveAllocation>[] = selected
    ? [resident, balance]
    : [
        resident,
        { header: 'Rate', cell: (s) => desk.formatMoney(s.rate_agreed), align: 'end' },
        paid,
        balance,
        {
          header: <span className="sr-only">Actions</span>,
          cell: (s) => (
            <Button size="sm" variant="secondary" onClick={() => onPay(s)}>
              Take payment
            </Button>
          ),
          align: 'end',
        },
      ];

  let body;
  if (stays.error && !all) {
    body = (
      <Alert title="Could not load balances" onRetry={stays.reload}>
        {stays.error}
      </Alert>
    );
  } else if (!rows || !all) {
    body = (
      <Card>
        <Skeleton rows={6} />
      </Card>
    );
  } else if (all.length === 0) {
    body = (
      <Card>
        <EmptyState
          icon="home"
          title="No active stays"
          description="Check a resident into a room to start taking payments."
          action={
            <Button icon="home" onClick={() => desk.navigate({ page: 'rooms' })}>
              Go to rooms
            </Button>
          }
        />
      </Card>
    );
  } else if (rows.length === 0) {
    body = (
      <Card>
        <EmptyState
          icon="check-circle"
          title="Everyone is paid up"
          description={`No active stay owes anything. ${all.length} ${all.length === 1 ? 'stay is' : 'stays are'} fully paid.`}
          action={
            <Button variant="secondary" onClick={() => setShowAll(true)}>
              Show all active stays
            </Button>
          }
        />
      </Card>
    );
  } else {
    body = (
      <Card flush>
        <Table
          caption="Active stays and what they owe"
          columns={columns}
          rows={rows}
          rowKey={(s) => s.allocation_id}
          selectedKey={selectedId}
          onRowClick={(s) => setSelectedId(s.allocation_id === selectedId ? null : s.allocation_id)}
          pageSize={10}
          noun="stays"
        />
      </Card>
    );
  }

  return (
    <>
      <div className="flex flex-wrap items-center justify-between gap-4">
        {tabs}
        <div className="shrink-0">
          <Toggle label="Include paid up" checked={showAll} onChange={setShowAll} />
        </div>
      </div>
      {body}
      {selected && (
        <StayPanel
          key={selected.allocation_id}
          stay={selected}
          onClose={() => setSelectedId(null)}
          onPay={() => onPay(selected)}
        />
      )}
    </>
  );
}

function StayPanel({
  stay,
  onClose,
  onPay,
}: {
  stay: ActiveAllocation;
  onClose: () => void;
  onPay: () => void;
}) {
  const desk = useDesk();
  const history = useLoad(
    () =>
      api.listPayments({
        hostel_id: desk.hostelId,
        from: null,
        to: null,
        resident_id: stay.resident_id,
        method: null,
      }),
    [desk.hostelId, stay.resident_id],
  );
  const rows = history.data?.filter((p) => p.allocation_id === stay.allocation_id);
  const partial = isOwing(stay.total_paid);

  return (
    <DetailPanel
      title={stay.resident_name}
      subtitle={stayLine(stay)}
      onClose={onClose}
      footer={
        <>
          <Button
            variant="secondary"
            icon="user-circle"
            onClick={() => desk.navigate({ page: 'residents', residentId: stay.resident_id })}
          >
            Resident
          </Button>
          <Button className="flex-1" icon="banknotes" onClick={onPay}>
            Take payment
          </Button>
        </>
      }
    >
      {/* ponytail: two tiles, the rate goes in the note below: three 7-digit amounts break mid-number in the 320 px panel (< 1280 px windows). */}
      <MiniStats
        items={[
          { label: 'Paid', value: desk.formatMoney(stay.total_paid) },
          { label: 'Balance', value: desk.formatMoney(stay.balance) },
        ]}
      />
      <Alert
        tone={!isOwing(stay.balance) ? 'success' : partial ? 'warning' : 'danger'}
        title={!isOwing(stay.balance) ? 'Paid up' : partial ? 'Part paid' : 'Nothing paid yet'}
      >
        Agreed rate {desk.formatMoney(stay.rate_agreed)} for {termLabel(stay.term)} {stay.academic_year}.
      </Alert>
      <DetailSection label="Payments for this stay">
        {history.error && !rows ? (
          <Alert title="Could not load payments" onRetry={history.reload}>
            {history.error}
          </Alert>
        ) : !rows ? (
          <Skeleton rows={3} />
        ) : rows.length === 0 ? (
          <p className="text-gray-500">No payments yet for this stay.</p>
        ) : (
          <ul className="divide-y divide-gray-200">
            {rows.map((p) => (
              <li key={p.id} className="flex items-center gap-3 py-3 first:pt-1">
                <div className="min-w-0 flex-1">
                  <p className="font-medium text-gray-900">{desk.formatDate(p.payment_date)}</p>
                  <p className="truncate text-gray-500">
                    {methodLabel(p.payment_method)}
                    {p.receipt_number && ` · ${p.receipt_number}`}
                  </p>
                </div>
                <span className="font-medium text-gray-900 tabular-nums">{desk.formatMoney(p.amount)}</span>
                {p.receipt_number && (
                  <Button
                    size="sm"
                    variant="ghost"
                    icon="document-text"
                    onClick={() => desk.showReceipt({ paymentId: p.id, receiptNumber: p.receipt_number ?? '' })}
                    aria-label={`Open receipt ${p.receipt_number}`}
                    title="Open receipt"
                  />
                )}
              </li>
            ))}
          </ul>
        )}
      </DetailSection>
    </DetailPanel>
  );
}

// ---------------------------------------------------------------- history

function History({
  tabs,
  payments,
  filter,
  setFilter,
  onShowOutstanding,
}: {
  tabs: ReactNode;
  payments: Load<PaymentRow[]>;
  filter: Filter;
  setFilter: (filter: Filter) => void;
  onShowOutstanding: () => void;
}) {
  const desk = useDesk();
  const filtered = filter.from !== '' || filter.to !== '' || filter.method !== '';
  const set = (key: keyof Filter, value: string) => setFilter({ ...filter, [key]: value });

  const columns: Column<PaymentRow>[] = [
    {
      header: 'Date',
      cell: (p) => (
        <>
          <p>{desk.formatDate(p.payment_date)}</p>
          <p className="text-sm font-normal text-gray-500 tabular-nums">
            {/* formatDateTime ends with the local "HH:mm" */}
            {desk.formatDateTime(p.payment_date).slice(-5)}
          </p>
        </>
      ),
    },
    { header: 'Receipt', cell: (p) => <span className="tabular-nums">{p.receipt_number ?? '—'}</span> },
    {
      header: 'Resident',
      cell: (p) => <NameCell name={p.resident_name} detail={`Room ${p.room_number}`} />,
    },
    { header: 'Method', cell: (p) => <Badge>{methodLabel(p.payment_method)}</Badge> },
    {
      header: 'Amount',
      cell: (p) => <span className="font-medium text-gray-900">{desk.formatMoney(p.amount)}</span>,
      align: 'end',
    },
    {
      header: <span className="sr-only">Actions</span>,
      cell: (p) =>
        p.receipt_number && (
          <Button
            size="sm"
            variant="secondary"
            icon="document-text"
            onClick={() => desk.showReceipt({ paymentId: p.id, receiptNumber: p.receipt_number ?? '' })}
          >
            Open receipt
          </Button>
        ),
      align: 'end',
    },
  ];

  let body;
  if (payments.error && !payments.data) {
    body = (
      <Alert title="Could not load payments" onRetry={payments.reload}>
        {payments.error}
      </Alert>
    );
  } else if (!payments.data) {
    body = (
      <Card>
        <Skeleton rows={6} />
      </Card>
    );
  } else if (payments.data.length === 0) {
    body = (
      <Card>
        {filtered ? (
          <EmptyState
            icon="magnifying-glass"
            title="No payments match these filters"
            description="Try other dates or another method."
            action={
              <Button variant="secondary" onClick={() => setFilter(NO_FILTER)}>
                Clear filters
              </Button>
            }
          />
        ) : (
          <EmptyState
            icon="banknotes"
            title="No payments yet"
            description="Payments you take show up here with their receipts."
            action={
              <Button icon="banknotes" onClick={onShowOutstanding}>
                See who owes
              </Button>
            }
          />
        )}
      </Card>
    );
  } else {
    body = (
      <Card flush>
        <Table
          caption="Payments, newest first"
          columns={columns}
          rows={payments.data}
          rowKey={(p) => p.id}
          pageSize={10}
          noun="payments"
        />
      </Card>
    );
  }

  return (
    <>
      <div className="flex flex-wrap items-end justify-between gap-4">
        {tabs}
        <div className="flex flex-wrap items-end gap-3">
          <Field label="From" className="w-40">
            <Input type="date" value={filter.from} max={filter.to || undefined} onChange={(e) => set('from', e.target.value)} />
          </Field>
          <Field label="To" className="w-40">
            <Input type="date" value={filter.to} min={filter.from || undefined} onChange={(e) => set('to', e.target.value)} />
          </Field>
          <Field label="Method" className="w-44">
            <Select value={filter.method} onChange={(e) => set('method', e.target.value)}>
              <option value="">All methods</option>
              {METHODS.map((m) => (
                <option key={m.value} value={m.value}>
                  {m.label}
                </option>
              ))}
            </Select>
          </Field>
          {filtered && (
            <Button variant="ghost" icon="x-mark" onClick={() => setFilter(NO_FILTER)}>
              Clear
            </Button>
          )}
        </div>
      </div>
      {body}
    </>
  );
}
