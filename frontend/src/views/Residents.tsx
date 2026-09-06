import { useEffect, useState, useCallback } from 'react';
import { Card } from '../components/ui/Card.tsx';
import { StatCard } from '../components/ui/StatCard.tsx';
import { Button } from '../components/ui/Button.tsx';
import { Badge } from '../components/ui/Badge.tsx';
import { Tabs } from '../components/ui/Tabs.tsx';
import { Modal } from '../components/ui/Modal.tsx';
import { Input } from '../components/ui/Input.tsx';
import { Table, Th, Td, Tr } from '../components/ui/Table.tsx';
import { Dropdown, DropdownItem } from '../components/ui/Dropdown.tsx';
import { EmptyState } from '../components/ui/EmptyState.tsx';
import { SearchInput } from '../components/ui/SearchInput.tsx';
import { DetailDrawer } from '../components/ui/DetailDrawer.tsx';
import { CurrencyAmount } from '../components/ui/CurrencyAmount.tsx';
import { CheckOutConfirm } from '../components/CheckOutConfirm.tsx';
import { CheckInForm } from '../components/CheckInForm.tsx';
import { useHostel } from '../hooks/use-hostel.ts';
import { formatDecimal, owesMoney } from '../lib/decimal.ts';
import { addToast } from '../lib/toasts.ts';
import {
  listResidents,
  getResidentDetail,
  updateResident,
  type ResidentSummary,
  type ResidentDetail,
} from '../lib/tauri.ts';
import type { View } from '../components/types';

type Props = { onNavigate: (v: View) => void };
type TabValue = 'current' | 'past' | 'all';

function initials(name: string): string {
  return name.split(' ').map((w) => w[0]).slice(0, 2).join('').toUpperCase();
}

function fmtDate(d?: string): string {
  if (!d) return '—';
  return new Date(d).toLocaleDateString('en-US', { month: 'short', day: 'numeric' });
}

const TERM_LABEL: Record<string, string> = {
  semester_1: 'Semester 1',
  semester_2: 'Semester 2',
  recess: 'Recess',
};

export function Residents({ onNavigate }: Props) {
  const { hostelId, currency } = useHostel();
  const [residents, setResidents] = useState<ResidentSummary[]>([]);
  const [loading, setLoading] = useState(true);
  const [refreshKey, setRefreshKey] = useState(0);
  const [query, setQuery] = useState('');
  const [tab, setTab] = useState<TabValue>('current');
  const [checkInOpen, setCheckInOpen] = useState(false);
  const [selectedId, setSelectedId] = useState<number | null>(null);
  const [detail, setDetail] = useState<ResidentDetail | null>(null);
  const [detailLoading, setDetailLoading] = useState(false);
  const [editOpen, setEditOpen] = useState(false);
  const [checkoutTarget, setCheckoutTarget] = useState<ResidentSummary | null>(null);

  const today = new Date().toLocaleDateString('en-CA');
  const refresh = useCallback(() => setRefreshKey((k) => k + 1), []);

  useEffect(() => {
    if (!hostelId) return;
    let cancelled = false;
    setLoading(true);
    listResidents({ hostel_id: hostelId })
      .then((r) => {
        if (!cancelled) setResidents(r);
      })
      .catch(() => {})
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [hostelId, refreshKey]);

  useEffect(() => {
    if (selectedId === null || !hostelId) return;
    let cancelled = false;
    setDetailLoading(true);
    setDetail(null);
    getResidentDetail({ hostel_id: hostelId, resident_id: selectedId })
      .then((d) => {
        if (!cancelled) setDetail(d);
      })
      .catch(() => {})
      .finally(() => {
        if (!cancelled) setDetailLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [selectedId, hostelId]);

  const filtered = residents.filter((r) => {
    if (tab === 'current' && r.status !== 'active') return false;
    if (tab === 'past' && r.status !== 'checked_out') return false;
    if (query.trim()) {
      const q = query.toLowerCase();
      return (
        r.full_name.toLowerCase().includes(q) ||
        (r.phone ?? '').toLowerCase().includes(q) ||
        (r.room_number ?? '').toLowerCase().includes(q)
      );
    }
    return true;
  });

  const tabs = [
    { value: 'current', label: 'Current', content: null },
    { value: 'past', label: 'Past', content: null },
    { value: 'all', label: 'All', content: null },
  ];

  const current = residents.filter((r) => r.status === 'active').length;
  const past = residents.filter((r) => r.status === 'checked_out').length;
  const overdue = residents.filter(
    (r) => r.status === 'active' && r.expected_checkout && r.expected_checkout < today,
  ).length;

  return (
    <div className="flex flex-col gap-4">
      <div className="flex items-center justify-between gap-4">
        <div>
          <h1 className="text-display-sm text-ink">Residents</h1>
          <p className="text-caption-sm text-muted">
            {filtered.length} {filtered.length === 1 ? 'record' : 'records'}
          </p>
        </div>
        <Button onClick={() => setCheckInOpen(true)}>
          New Resident
        </Button>
      </div>

      <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
        <StatCard label="Total residents" value={residents.length} />
        <StatCard label="Current" value={current} />
        <StatCard label="Past" value={past} />
        <StatCard label="Overdue" value={overdue} error={overdue > 0} />
      </div>

      <div className="grid grid-cols-[1fr_20rem_1fr] items-center gap-4">
        <div />
        <SearchInput
          placeholder="Search name, phone, room…"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          className="w-full"
        />
        <div className="flex justify-end">
          <Tabs tabs={tabs} value={tab} onValueChange={(v) => setTab(v as TabValue)} />
        </div>
      </div>

      {loading ? (
        <Card shadow className="p-5">
          <div className="flex flex-col gap-3">
            {[...Array(5)].map((_, i) => (
              <div key={i} className="h-10 animate-pulse rounded-md bg-surface-soft" />
            ))}
          </div>
        </Card>
      ) : residents.length === 0 ? (
        <EmptyState
          title="No resident records yet"
          description="Check in your first resident to get started."
          action={<Button onClick={() => setCheckInOpen(true)}>New Resident</Button>}
        />
      ) : filtered.length === 0 ? (
        <Card shadow className="p-5 text-center">
          <p className="text-body-sm text-muted">No matches for "{query}"</p>
        </Card>
      ) : (
        <Table>
          <thead>
            <tr>
              <Th>Resident</Th>
              <Th>Room</Th>
              <Th>Checked in</Th>
              <Th>Expected out</Th>
              <Th>Balance</Th>
              <Th className="w-8" />
            </tr>
          </thead>
          <tbody>
            {filtered.map((r) => {
              const overdue = r.status === 'active' && r.expected_checkout && r.expected_checkout < today;
              const owes = owesMoney(r.balance);
              return (
                <Tr
                  key={r.id}
                  className="cursor-pointer"
                  onClick={() => setSelectedId(r.id)}
                >
                  <Td>
                    <div className="flex items-center gap-3">
                      <span className="flex h-8 w-8 items-center justify-center rounded-full bg-surface-strong text-caption text-ink">
                        {initials(r.full_name)}
                      </span>
                      <span>{r.full_name}</span>
                    </div>
                  </Td>
                  <Td>{r.room_number ?? '—'}</Td>
                  <Td>{fmtDate(r.check_in_date)}</Td>
                  <Td>
                    <span className="flex items-center gap-2">
                      {fmtDate(r.expected_checkout)}
                      {overdue && <Badge className="bg-error-text text-on-primary">Overdue</Badge>}
                    </span>
                  </Td>
                  <Td>
                    <CurrencyAmount amount={r.balance} currency={currency} error={owes} />
                  </Td>
                  <Td onClick={(e) => e.stopPropagation()}>
                    <Dropdown
                      trigger={
                        <button className="rounded-full p-1.5 text-muted hover:bg-surface-soft hover:text-ink">
                          ⋯
                        </button>
                      }
                    >
                      <DropdownItem onSelect={() => setSelectedId(r.id)}>View</DropdownItem>
                      <DropdownItem onSelect={() => { setSelectedId(r.id); setEditOpen(true); }}>Edit</DropdownItem>
                      {r.status === 'active' && r.allocation_id && (
                        <DropdownItem onSelect={() => setCheckoutTarget(r)}>Check out</DropdownItem>
                      )}
                    </Dropdown>
                  </Td>
                </Tr>
              );
            })}
          </tbody>
        </Table>
      )}

      <Modal
        open={checkInOpen}
        onOpenChange={setCheckInOpen}
        title="Check in resident"
        description="Assign a vacant room to a new resident."
      >
        <CheckInForm
          onSuccess={() => {
            setCheckInOpen(false);
            refresh();
          }}
        />
      </Modal>

      <DetailDrawer
        open={selectedId !== null}
        onOpenChange={(o) => !o && setSelectedId(null)}
      >
        {detailLoading ? (
          <div className="flex flex-col gap-3 pt-8">
            {[...Array(6)].map((_, i) => (
              <div key={i} className="h-5 animate-pulse rounded bg-surface-soft" />
            ))}
          </div>
        ) : detail ? (
          <ResidentDrawerContent
            detail={detail}
            currency={currency}
            hostelId={hostelId}
            onNavigate={onNavigate}
            onCheckOut={() => {
              setEditOpen(false);
              setSelectedId(null);
              setCheckoutTarget({
                id: detail.resident.id,
                full_name: detail.resident.full_name,
                phone: detail.resident.phone,
                email: detail.resident.email,
                allocation_id: detail.allocations[0]?.id,
                room_number: detail.allocations[0]?.room_number,
                check_in_date: detail.allocations[0]?.check_in_date,
                expected_checkout: detail.allocations[0]?.expected_checkout,
                status: detail.allocations[0]?.status ?? 'active',
                balance: detail.balance,
              });
            }}
            onEdit={() => setEditOpen(true)}
          />
        ) : null}
      </DetailDrawer>

      {detail && (
        <EditResidentModal
        hostelId={hostelId}
          open={editOpen}
          onOpenChange={setEditOpen}
          detail={detail}
          onDone={() => {
            setEditOpen(false);
            refresh();
            if (selectedId !== null) {
              getResidentDetail({ hostel_id: hostelId, resident_id: selectedId })
                .then(setDetail)
                .catch(() => {});
            }
          }}
        />
      )}

      {checkoutTarget && checkoutTarget.allocation_id && (
        <CheckOutConfirm
          open={!!checkoutTarget}
          onOpenChange={(o) => !o && setCheckoutTarget(null)}
          hostelId={hostelId}
          currency={currency}
          allocationId={checkoutTarget.allocation_id}
          residentName={checkoutTarget.full_name}
          roomNumber={checkoutTarget.room_number ?? '—'}
          balance={checkoutTarget.balance}
          onDone={() => {
            setCheckoutTarget(null);
            refresh();
          }}
        />
      )}
    </div>
  );
}

function ResidentDrawerContent({
  detail,
  currency,
  onNavigate,
  onCheckOut,
  onEdit,
}: {
  detail: ResidentDetail;
  currency: string;
  hostelId: number;
  onNavigate: (v: View) => void;
  onCheckOut: () => void;
  onEdit: () => void;
}) {
  const active = detail.allocations.find((a) => a.status === 'active');
  const owes = owesMoney(detail.balance);

  return (
    <div className="flex flex-col gap-5 pt-8">
      <div className="flex items-center gap-3">
        <span className="flex h-12 w-12 items-center justify-center rounded-full bg-surface-strong text-title-sm text-ink">
          {initials(detail.resident.full_name)}
        </span>
        <div>
          <h2 className="text-display-sm text-ink">{detail.resident.full_name}</h2>
          {active && <Badge className="bg-surface-strong text-muted">Active</Badge>}
        </div>
      </div>

      <div className="flex flex-col gap-1 text-body-sm text-muted">
        {detail.resident.phone && <p>{detail.resident.phone}</p>}
        {detail.resident.email && <p>{detail.resident.email}</p>}
        {detail.resident.id_number && <p>ID: {detail.resident.id_number}</p>}
        {detail.resident.next_of_kin && <p>Next of kin: {detail.resident.next_of_kin}</p>}
      </div>

      {active && (
        <Card shadow className="p-4">
          <p className="text-caption text-muted">Current stay</p>
          <p className="mt-1 text-body-sm text-ink">
            {active.room_number} · {fmtDate(active.check_in_date)} → {fmtDate(active.expected_checkout)}
          </p>
          <p className="text-caption-sm text-muted">
            {TERM_LABEL[active.term] ?? active.term} · {active.academic_year}
          </p>
          <p className="mt-2 text-caption text-muted">Balance</p>
          <p className={`text-display-md ${owes ? 'text-error-text' : 'text-ink'}`}>
            {currency} {formatDecimal(detail.balance)}
          </p>
          <div className="mt-4 flex gap-3">
            <Button variant="secondary" size="sm" onClick={() => onNavigate('invoices')}>
              Record payment
            </Button>
            <Button variant="tertiary" size="sm" onClick={onCheckOut}>
              Check out
            </Button>
          </div>
        </Card>
      )}

      <div>
        <p className="mb-2 text-caption text-muted">Stay history</p>
        {detail.allocations.length === 0 ? (
          <p className="text-body-sm text-muted">No stays recorded</p>
        ) : (
          <div className="flex flex-col gap-2">
            {detail.allocations.map((a) => (
              <div key={a.id} className="flex items-center justify-between text-body-sm">
                <span className="text-ink">
                  {a.room_number}
                  <span className="ml-2 text-caption-sm text-muted">
                    {TERM_LABEL[a.term] ?? a.term} · {a.academic_year}
                  </span>
                </span>
                <span className="text-muted">
                  {fmtDate(a.check_in_date)} → {a.check_out_date ? fmtDate(a.check_out_date) : '—'}
                </span>
                <Badge className="bg-surface-strong text-muted">{a.status}</Badge>
              </div>
            ))}
          </div>
        )}
      </div>

      <div>
        <p className="mb-2 text-caption text-muted">Payment history</p>
        {detail.payments.length === 0 ? (
          <p className="text-body-sm text-muted">No payments recorded</p>
        ) : (
          <div className="flex flex-col gap-2">
            {detail.payments.map((p) => (
              <div key={p.id} className="flex items-center justify-between text-body-sm">
                <span className="text-ink">{currency} {formatDecimal(p.amount)}</span>
                <span className="text-muted">{fmtDate(p.payment_date)}</span>
                <span className="text-muted capitalize">{p.payment_method}</span>
              </div>
            ))}
          </div>
        )}
      </div>

      <div className="border-t border-hairline pt-4">
        <Button variant="tertiary" size="sm" onClick={onEdit}>
          Edit details
        </Button>
      </div>
    </div>
  );
}

function EditResidentModal({
  open,
  onOpenChange,
  hostelId,
  detail,
  onDone,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  hostelId: number;
  detail: ResidentDetail;
  onDone: () => void;
}) {
  const [fullName, setFullName] = useState(detail.resident.full_name);
  const [phone, setPhone] = useState(detail.resident.phone ?? '');
  const [email, setEmail] = useState(detail.resident.email ?? '');
  const [idNumber, setIdNumber] = useState(detail.resident.id_number ?? '');
  const [nextOfKin, setNextOfKin] = useState(detail.resident.next_of_kin ?? '');
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    setFullName(detail.resident.full_name);
    setPhone(detail.resident.phone ?? '');
    setEmail(detail.resident.email ?? '');
    setIdNumber(detail.resident.id_number ?? '');
    setNextOfKin(detail.resident.next_of_kin ?? '');
  }, [detail]);

  const nameValid = fullName.trim().length > 0;

  async function handleSave(e: React.FormEvent) {
    e.preventDefault();
    if (!nameValid) return;
    setBusy(true);
    try {
      await updateResident({
        hostel_id: hostelId,
        resident_id: detail.resident.id,
        full_name: fullName.trim(),
        phone: phone.trim() || undefined,
        email: email.trim() || undefined,
        id_number: idNumber.trim() || undefined,
        next_of_kin: nextOfKin.trim() || undefined,
      });
      addToast('Resident updated.', 'success');
      onDone();
    } catch {
      // toast already surfaced
    } finally {
      setBusy(false);
    }
  }

  return (
    <Modal
      open={open}
      onOpenChange={onOpenChange}
      title="Edit resident"
      description="Update resident details."
    >
      <form onSubmit={handleSave} className="flex flex-col gap-4">
        <Input
          label="Full name"
          value={fullName}
          onChange={(e) => setFullName(e.target.value)}
          required
          error={!nameValid ? 'Name is required' : undefined}
        />
        <Input label="Phone" type="tel" value={phone} onChange={(e) => setPhone(e.target.value)} />
        <Input label="Email" type="email" value={email} onChange={(e) => setEmail(e.target.value)} />
        <Input label="ID number" value={idNumber} onChange={(e) => setIdNumber(e.target.value)} />
        <Input label="Next of kin" value={nextOfKin} onChange={(e) => setNextOfKin(e.target.value)} />
        <div className="flex justify-end">
          <Button type="submit" disabled={busy || !nameValid}>
            {busy ? 'Saving…' : 'Save'}
          </Button>
        </div>
      </form>
    </Modal>
  );
}
