import { useEffect, useState } from 'react';
import { Button } from '../components/ui/Button.tsx';
import { Tabs } from '../components/ui/Tabs.tsx';
import { Modal } from '../components/ui/Modal.tsx';
import { Input } from '../components/ui/Input.tsx';
import { Select } from '../components/ui/Select.tsx';
import { EmptyState } from '../components/ui/EmptyState.tsx';
import { InvoiceIcon, PlusIcon } from '../components/ui/icons.tsx';
import { addToast } from '../lib/toasts.ts';
import {
  recordPaymentWithReceipt,
  generateAndOpenReceipt,
  listHostels,
  type Hostel,
} from '../lib/tauri.ts';

type Tab = 'all' | 'paid' | 'unpaid';

const METHODS = [
  { value: 'cash', label: 'Cash' },
  { value: 'mobile_money', label: 'Mobile money' },
  { value: 'bank_transfer', label: 'Bank transfer' },
  { value: 'card', label: 'Card' },
];

export function Invoices() {
  const [tab, setTab] = useState<Tab>('all');
  const [recordOpen, setRecordOpen] = useState(false);
  const [hostel, setHostel] = useState<Hostel | null>(null);

  const [residentName, setResidentName] = useState('');
  const [amount, setAmount] = useState('');
  const [method, setMethod] = useState('cash');
  const [allocationId, setAllocationId] = useState('');
  const [residentId, setResidentId] = useState('');
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    listHostels()
      .then((h) => setHostel(h[0] ?? null))
      .catch(() => setHostel(null));
  }, []);

  const valid =
    /^\d+(\.\d+)?$/.test(amount.trim()) &&
    residentName.trim().length > 0 &&
    Number(allocationId) > 0 &&
    Number(residentId) > 0;

  async function handleSubmit(e: React.FormEvent) {
    e.preventDefault();
    if (!hostel || !valid) {
      addToast('Fill all fields with valid values.', 'error');
      return;
    }
    setBusy(true);
    try {
      const [, receiptNumber] = await recordPaymentWithReceipt({
        hostel_id: hostel.id,
        allocation_id: Number(allocationId),
        resident_id: Number(residentId),
        amount: amount.trim(),
        payment_method: method,
      });
      addToast(`Payment recorded. Receipt #${receiptNumber}.`, 'success');
      await generateAndOpenReceipt({
        hostel_name: hostel.name,
        receipt_number: Number(receiptNumber),
        resident_name: residentName.trim(),
        amount: amount.trim(),
        date: new Date().toISOString().split('T')[0],
        balance: '0.00',
      });
      setRecordOpen(false);
      setAmount('');
      setResidentName('');
      setAllocationId('');
      setResidentId('');
    } catch {
      // tauri wrapper toasts the error
    } finally {
      setBusy(false);
    }
  }

  const tabs = [
    { value: 'all', label: 'All', content: null },
    { value: 'paid', label: 'Paid', content: null },
    { value: 'unpaid', label: 'Unpaid', content: null },
  ];

  return (
    <div className="flex flex-col gap-6">
      <div className="flex items-center justify-between gap-4">
        <Tabs tabs={tabs} value={tab} onValueChange={(v) => setTab(v as Tab)} />
        <Button onClick={() => setRecordOpen(true)}>
          <PlusIcon width={16} height={16} />
          Record payment
        </Button>
      </div>

      {/* ponytail: list_payments command pending — empty state until then. */}
      <EmptyState
        title="No invoices yet"
        description="Record a payment to generate a receipt. Past invoices will be listed here once the payments list command ships."
        icon={<InvoiceIcon width={22} height={22} />}
        action={
          <Button variant="secondary" onClick={() => setRecordOpen(true)}>
            Record a payment
          </Button>
        }
      />

      <Modal
        open={recordOpen}
        onOpenChange={setRecordOpen}
        title="Record payment"
        description="Records the payment and generates a receipt PDF."
      >
        <form onSubmit={handleSubmit} className="flex flex-col gap-4">
          <Input
            label="Resident name"
            value={residentName}
            onChange={(e) => setResidentName(e.target.value)}
            placeholder="Jane Doe"
          />
          <div className="grid gap-4 sm:grid-cols-2">
            <Input
              label="Resident ID"
              type="number"
              value={residentId}
              onChange={(e) => setResidentId(e.target.value)}
              placeholder="1"
            />
            <Input
              label="Allocation ID"
              type="number"
              value={allocationId}
              onChange={(e) => setAllocationId(e.target.value)}
              placeholder="1"
            />
          </div>
          <div className="grid gap-4 sm:grid-cols-2">
            <Input
              label="Amount (UGX)"
              inputMode="decimal"
              value={amount}
              onChange={(e) => setAmount(e.target.value)}
              placeholder="50000.00"
            />
            <Select
              label="Method"
              value={method}
              capitalize
              onChange={(e) => setMethod(e.target.value)}
            >
              {METHODS.map((m) => (
                <option key={m.value} value={m.value}>
                  {m.label}
                </option>
              ))}
            </Select>
          </div>
          <div className="flex justify-end gap-3">
            <Button
              type="button"
              variant="tertiary"
              onClick={() => setRecordOpen(false)}
            >
              Cancel
            </Button>
            <Button type="submit" disabled={busy || !valid}>
              {busy ? 'Recording…' : 'Record & generate receipt'}
            </Button>
          </div>
        </form>
      </Modal>
    </div>
  );
}
