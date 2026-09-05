import { useEffect, useState, useMemo, useCallback } from 'react';
import { Card } from '../components/ui/Card.tsx';
import { Button } from '../components/ui/Button.tsx';
import { Tabs } from '../components/ui/Tabs.tsx';
import { Modal } from '../components/ui/Modal.tsx';
import { Input } from '../components/ui/Input.tsx';
import { Select } from '../components/ui/Select.tsx';
import { Textarea } from '../components/ui/Textarea.tsx';
import { Table, Th, Td, Tr } from '../components/ui/Table.tsx';
import { EmptyState } from '../components/ui/EmptyState.tsx';
import { SearchInput } from '../components/ui/SearchInput.tsx';
import { CurrencyAmount } from '../components/ui/CurrencyAmount.tsx';
import { Dropdown, DropdownItem } from '../components/ui/Dropdown.tsx';
import { useHostel } from '../hooks/use-hostel.ts';
import { addToast } from '../lib/toasts.ts';
import {
  addDecimalStrings,
  formatDecimal,
  isPositiveDecimal,
  subtractDecimalStrings,
} from '../lib/decimal.ts';
import {
  listPayments,
  listActiveAllocations,
  recordPaymentWithReceipt,
  generateAndOpenReceipt,
  getAppPreferences,
  getResidentDetail,
  type AppPreferences,
  type PaymentRow,
  type ActiveAllocation,
} from '../lib/tauri.ts';
import type { View } from '../components/types';

type Props = { onNavigate: (v: View) => void };
type TabValue = 'ledger' | 'outstanding';

const METHODS = [
  { value: '', label: 'All methods' },
  { value: 'cash', label: 'Cash' },
  { value: 'bank_transfer', label: 'Bank transfer' },
];

function monthLabel(d: Date): string {
  return d.toLocaleDateString('en-US', { month: 'long', year: 'numeric' });
}

function monthKey(iso: string): string {
  return iso.slice(0, 7);
}

export function Invoices({ onNavigate: _onNavigate }: Props) {
  const { hostel, hostelId, currency } = useHostel();
  const [tab, setTab] = useState<TabValue>('ledger');
  const [payments, setPayments] = useState<PaymentRow[]>([]);
  const [allocations, setAllocations] = useState<ActiveAllocation[]>([]);
  const [loading, setLoading] = useState(true);
  const [refreshKey, setRefreshKey] = useState(0);
  const [query, setQuery] = useState('');
  const [methodFilter, setMethodFilter] = useState('');
  const [monthFilter, setMonthFilter] = useState('');
  const [recordOpen, setRecordOpen] = useState(false);
  const [preferences, setPreferences] = useState<AppPreferences | null>(null);

  const refresh = useCallback(() => setRefreshKey((k) => k + 1), []);

  useEffect(() => {
    if (!hostelId) return;
    let cancelled = false;
    setLoading(true);
    Promise.all([
      listPayments({ hostel_id: hostelId }),
      listActiveAllocations({ hostel_id: hostelId }),
      getAppPreferences({ hostel_id: hostelId }),
    ])
      .then(([p, a, prefs]) => {
        if (cancelled) return;
        setPayments(p);
        setAllocations(a);
        setPreferences(prefs);
      })
      .catch(() => {})
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [hostelId, refreshKey]);

  const months = useMemo(() => {
    const set = new Map<string, string>();
    const now = new Date();
    set.set('', 'All months');
    for (let i = 0; i < 12; i++) {
      const d = new Date(now.getFullYear(), now.getMonth() - i, 1);
      set.set(`${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}`, monthLabel(d));
    }
    return [...set.entries()];
  }, []);

  const filteredPayments = useMemo(() => {
    return payments.filter((p) => {
      if (methodFilter && p.payment_method !== methodFilter) return false;
      if (monthFilter && monthKey(p.payment_date) !== monthFilter) return false;
      if (query.trim()) {
        const q = query.toLowerCase();
        return (
          p.resident_name.toLowerCase().includes(q) ||
          p.room_number.toLowerCase().includes(q) ||
          (p.receipt_number ?? '').toLowerCase().includes(q)
        );
      }
      return true;
    });
  }, [payments, methodFilter, monthFilter, query]);

  const monthTotal = useMemo(() => {
    return filteredPayments.reduce((acc, p) => addDecimalStrings(acc, p.amount), '0');
  }, [filteredPayments]);

  const outstanding = useMemo(() => {
    return allocations.filter((a) => {
      const bal = BigInt(a.balance.replace('.', '').replace(/[^0-9]/g, '') || '0');
      return bal > 0n;
    });
  }, [allocations]);

  const filteredOutstanding = useMemo(() => {
    if (!query.trim()) return outstanding;
    const q = query.toLowerCase();
    return outstanding.filter(
      (a) => a.resident_name.toLowerCase().includes(q) || a.room_number.toLowerCase().includes(q),
    );
  }, [outstanding, query]);

  const tabs = [
    { value: 'ledger', label: 'Ledger', content: null },
    { value: 'outstanding', label: 'Outstanding', content: null },
  ];

  function exportCsv() {
    const rows = filteredPayments;
    const header = 'Date,Receipt #,Resident,Room,Method,Amount,Notes\n';
    const body = rows.map((p) =>
      [
        new Date(p.payment_date).toLocaleDateString(),
        p.receipt_number ?? '',
        `"${p.resident_name.replace(/"/g, '""')}"`,
        p.room_number,
        p.payment_method,
        p.amount,
        `"${(p.notes ?? '').replace(/"/g, '""')}"`,
      ].join(','),
    ).join('\n');
    const csv = header + body;
    const blob = new Blob([csv], { type: 'text/csv' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `payments-${new Date().toLocaleDateString('en-CA')}.csv`;
    a.click();
    URL.revokeObjectURL(url);
  }

  return (
    <div className="flex flex-col gap-4">
      <div className="flex items-center justify-between gap-4">
        <div>
          <h1 className="text-display-sm text-ink">Payments</h1>
          <p className="text-caption-sm text-muted">
            {currency} {formatDecimal(monthTotal)} collected this month
          </p>
        </div>
        <Button onClick={() => setRecordOpen(true)}>
          Record payment
        </Button>
      </div>

      <div className="grid grid-cols-[1fr_20rem_1fr] items-center gap-4">
        <Tabs tabs={tabs} value={tab} onValueChange={(v) => setTab(v as TabValue)} />
        <SearchInput
          placeholder="Search resident, room, receipt…"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          className="w-full"
        />
        <div className="flex items-center justify-end gap-3">
          <Select value={methodFilter} onChange={(e) => setMethodFilter(e.target.value)} className="w-auto">
            {METHODS.map((m) => (
              <option key={m.value} value={m.value}>{m.label}</option>
            ))}
          </Select>
          <Select value={monthFilter} onChange={(e) => setMonthFilter(e.target.value)} className="w-auto">
            {months.map(([v, label]) => (
              <option key={v} value={v}>{label}</option>
            ))}
          </Select>
        </div>
      </div>

      {tab === 'ledger' && (
        <>
          {loading ? (
            <Card shadow className="p-5">
              <div className="flex flex-col gap-3">
                {[...Array(5)].map((_, i) => (
                  <div key={i} className="h-10 animate-pulse rounded-md bg-surface-soft" />
                ))}
              </div>
            </Card>
          ) : filteredPayments.length === 0 ? (
            <EmptyState
              title="No payments recorded yet"
              description="Record a payment to generate a receipt."
              action={<Button onClick={() => setRecordOpen(true)}>Record a payment</Button>}
            />
          ) : (
            <>
              <Table>
                <thead>
                  <tr>
                    <Th>Date</Th>
                    <Th>Receipt #</Th>
                    <Th>Resident</Th>
                    <Th>Room</Th>
                    <Th>Method</Th>
                    <Th className="text-right">Amount</Th>
                    <Th className="w-8" />
                  </tr>
                </thead>
                <tbody>
                  {filteredPayments.map((p) => (
                    <Tr key={p.id}>
                      <Td>{new Date(p.payment_date).toLocaleDateString()}</Td>
                      <Td>{p.receipt_number ?? '—'}</Td>
                      <Td>{p.resident_name}</Td>
                      <Td>{p.room_number}</Td>
                      <Td className="capitalize">{p.payment_method}</Td>
                      <Td className="text-right">
                        <CurrencyAmount amount={p.amount} currency={currency} />
                      </Td>
                      <Td>
                        <Dropdown
                          trigger={
                            <button className="rounded-full p-1.5 text-muted hover:bg-surface-soft hover:text-ink">⋯</button>
                          }
                        >
                          <DropdownItem onSelect={() => handleViewReceipt(p)}>View receipt</DropdownItem>
                          <DropdownItem onSelect={() => navigator.clipboard.writeText(p.receipt_number ?? '')}>
                            Copy receipt #
                          </DropdownItem>
                        </Dropdown>
                      </Td>
                    </Tr>
                  ))}
                </tbody>
              </Table>
              <div className="flex justify-end">
                <Button variant="tertiary" size="sm" onClick={exportCsv}>Export CSV</Button>
              </div>
            </>
          )}
        </>
      )}

      {tab === 'outstanding' && (
        <>
          {loading ? (
            <Card shadow className="p-5">
              <div className="flex flex-col gap-3">
                {[...Array(3)].map((_, i) => (
                  <div key={i} className="h-10 animate-pulse rounded-md bg-surface-soft" />
                ))}
              </div>
            </Card>
          ) : filteredOutstanding.length === 0 ? (
            <EmptyState
              title="Everyone is paid up"
              description="No outstanding balances right now."
            />
          ) : (
            <Table>
              <thead>
                <tr>
                  <Th>Resident</Th>
                  <Th>Room</Th>
                  <Th>Since</Th>
                  <Th>Rate</Th>
                  <Th>Paid</Th>
                  <Th>Balance</Th>
                  <Th>Action</Th>
                </tr>
              </thead>
              <tbody>
                {filteredOutstanding.map((a) => (
                  <Tr key={a.allocation_id}>
                    <Td>{a.resident_name}</Td>
                    <Td>{a.room_number}</Td>
                    <Td>{new Date(a.check_in_date).toLocaleDateString()}</Td>
                    <Td><CurrencyAmount amount={a.rate_agreed} currency={currency} muted /></Td>
                    <Td><CurrencyAmount amount={a.total_paid} currency={currency} muted /></Td>
                    <Td><CurrencyAmount amount={a.balance} currency={currency} error /></Td>
                    <Td>
                      <Button
                        variant="tertiary"
                        size="sm"
                        onClick={() => setRecordOpen(true)}
                      >
                        Record
                      </Button>
                    </Td>
                  </Tr>
                ))}
              </tbody>
            </Table>
          )}
        </>
      )}

      <RecordPaymentModal
        open={recordOpen}
        onOpenChange={setRecordOpen}
        hostelId={hostelId}
        hostelName={hostel?.name || 'HostelDesk'}
        logoPath={hostel?.branding_logo_path || null}
        receiptFooter={preferences?.receipt_footer || null}
        currency={currency}
        allocations={allocations}
        onDone={refresh}
      />
    </div>
  );

  async function handleViewReceipt(p: PaymentRow) {
    try {
      const detail = await getResidentDetail({
        hostel_id: hostelId,
        resident_id: p.resident_id,
      });
      await generateAndOpenReceipt({
        hostel_id: hostelId,
        hostel_name: hostel?.name || 'HostelDesk',
        receipt_number: Number(p.receipt_number ?? 0),
        resident_name: p.resident_name,
        amount: p.amount,
        date: p.payment_date.slice(0, 10),
        balance: detail.balance,
        currency_code: currency,
        footer_text: preferences?.receipt_footer || null,
        logo_path: hostel?.branding_logo_path || null,
      });
    } catch {
      // toast already surfaced
    }
  }
}

function RecordPaymentModal({
  open,
  onOpenChange,
  hostelId,
  hostelName,
  logoPath,
  receiptFooter,
  currency,
  allocations,
  onDone,
}: {
  open: boolean;
  onOpenChange: (o: boolean) => void;
  hostelId: number;
  hostelName: string;
  logoPath: string | null;
  receiptFooter: string | null;
  currency: string;
  allocations: ActiveAllocation[];
  onDone: () => void;
}) {
  const [allocId, setAllocId] = useState('');
  const [amount, setAmount] = useState('');
  const [method, setMethod] = useState('cash');
  const [notes, setNotes] = useState('');
  const [busy, setBusy] = useState(false);

  const selected = allocations.find((a) => a.allocation_id === Number(allocId));
  const amountValid = isPositiveDecimal(amount);

  useEffect(() => {
    if (selected) {
      setAmount(selected.balance);
    } else {
      setAmount('');
    }
  }, [selected]);

  async function handleSubmit(e: React.FormEvent) {
    e.preventDefault();
    if (!selected || !amountValid || !hostelId) return;
    setBusy(true);
    try {
      const [paymentId, receiptNumber] = await recordPaymentWithReceipt({
        hostel_id: hostelId,
        allocation_id: selected.allocation_id,
        resident_id: selected.resident_id,
        amount: amount.trim(),
        payment_method: method,
        notes: notes.trim() || undefined,
      });
      addToast(`Payment recorded. Receipt #${receiptNumber}.`, 'success');
      try {
        await generateAndOpenReceipt({
          hostel_id: hostelId,
          hostel_name: hostelName,
          receipt_number: Number(receiptNumber),
          resident_name: selected.resident_name,
          amount: amount.trim(),
          date: new Date().toLocaleDateString('en-CA'),
          balance: (() => {
            const remaining = subtractDecimalStrings(selected.balance, amount.trim());
            return remaining.startsWith('-') ? '0' : remaining;
          })(),
          currency_code: currency,
          footer_text: receiptFooter,
          logo_path: logoPath,
        });
      } catch {
        // receipt generation failure is non-fatal — payment already recorded
      }
      setAllocId('');
      setAmount('');
      setNotes('');
      onOpenChange(false);
      onDone();
      void paymentId;
    } catch {
      // toast already surfaced
    } finally {
      setBusy(false);
    }
  }

  return (
    <Modal open={open} onOpenChange={onOpenChange} title="Record payment" description="Records the payment and generates a receipt PDF.">
      {allocations.length === 0 ? (
        <EmptyState
          title="No active stays"
          description="Check in a resident first before recording payments."
        />
      ) : (
        <form onSubmit={handleSubmit} className="flex flex-col gap-4">
          <Select
            label="Resident"
            value={allocId}
            onChange={(e) => setAllocId(e.target.value)}
            required
          >
            <option value="">Select a resident</option>
            {allocations.map((a) => (
              <option key={a.allocation_id} value={a.allocation_id}>
                {a.resident_name} · {a.room_number}
              </option>
            ))}
          </Select>

          {selected && (
            <div className="rounded-md border border-hairline p-3">
              <div className="flex justify-between text-body-sm">
                <span className="text-muted">Rate</span>
                <span className="text-ink">{currency} {formatDecimal(selected.rate_agreed)}</span>
              </div>
              <div className="flex justify-between text-body-sm">
                <span className="text-muted">Paid so far</span>
                <span className="text-ink">{currency} {formatDecimal(selected.total_paid)}</span>
              </div>
              <div className="flex justify-between text-body-sm">
                <span className="text-muted">Balance</span>
                <span className="text-error-text">{currency} {formatDecimal(selected.balance)}</span>
              </div>
            </div>
          )}

          <Input
            label="Amount"
            inputMode="decimal"
            value={amount}
            onChange={(e) => setAmount(e.target.value)}
            placeholder="0.00"
          />

          <Select label="Method" value={method} capitalize onChange={(e) => setMethod(e.target.value)}>
            <option value="cash">Cash</option>
            <option value="bank_transfer">Bank transfer</option>
          </Select>

          <Textarea label="Notes (optional)" value={notes} onChange={(e) => setNotes(e.target.value)} rows={2} />

          <div className="flex justify-end">
            <Button type="submit" disabled={busy || !allocId || !amountValid}>
              {busy ? 'Recording…' : 'Record & generate receipt'}
            </Button>
          </div>
        </form>
      )}
    </Modal>
  );
}
