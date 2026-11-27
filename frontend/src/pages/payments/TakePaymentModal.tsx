import { useState, type FormEvent } from 'react';
import { Alert, Button, Field, Input, MiniStats, Modal, Textarea } from '../../components/ui';
import * as api from '../../lib/api';
import { useDesk } from '../../lib/desk';
import { blank, METHODS, termLabel } from '../../lib/format';
import { cleanMoney, isMoneyInput, isOwing } from '../../lib/money';
import type { ActiveAllocation } from '../../lib/types';
import { useAction } from '../../lib/useLoad';

/** Props shared by every caller (Payments page, resident detail). */
export interface TakePaymentModalProps {
  /** Open the dialog for this stay; `null` keeps it closed. */
  stay: ActiveAllocation | null;
  /** Called after closing, whether or not a payment was taken. */
  onClose: () => void;
}

const AMOUNT_ERROR = 'Amount must be above zero, like 150000.';

/**
 * Take a payment for one active stay and print its receipt (`take_payment`).
 * Rust validates the amount again, records it, works out the balance after it
 * and opens the receipt PDF; this dialog does no money arithmetic.
 */
export default function TakePaymentModal({ stay, onClose }: TakePaymentModalProps) {
  const desk = useDesk();
  const pay = useAction();
  const [amountError, setAmountError] = useState<string | null>(null);

  const close = () => {
    setAmountError(null);
    pay.setError(null);
    onClose();
  };

  // ponytail: uncontrolled fields read with FormData. Modal mounts its content only
  // while open, so every open starts fresh and a background reload never touches typing.
  const submit = (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    if (!stay) return;
    const data = new FormData(event.currentTarget);
    const amount = cleanMoney(String(data.get('amount') ?? ''));
    if (!isMoneyInput(amount)) {
      setAmountError(AMOUNT_ERROR);
      return;
    }
    setAmountError(null);
    const method = METHODS.find((m) => m.value === data.get('method'))?.value ?? 'cash';
    void pay.run(async () => {
      const result = await api.takePayment({
        hostel_id: desk.hostelId,
        allocation_id: stay.allocation_id,
        amount,
        payment_method: method,
        notes: blank(String(data.get('notes') ?? '')),
      });
      if (result.pdf_error === null) {
        desk.toast({ tone: 'success', title: `Payment recorded. Receipt ${result.receipt_number} opened.` });
      } else {
        desk.toast({
          tone: 'warning',
          // Kept short: the toast wraps long titles and would split the receipt number at its hyphen.
          title: 'Payment recorded, but the PDF failed',
          message: `Receipt ${result.receipt_number}: ${result.pdf_error}`,
        });
      }
      close();
      desk.reload();
    });
  };

  const owing = stay !== null && isOwing(stay.balance);

  return (
    <Modal
      open={stay !== null}
      onClose={close}
      onSubmit={submit}
      busy={pay.busy}
      title="Take payment"
      description={
        stay &&
        `${stay.resident_name} · Room ${stay.room_number} · ${termLabel(stay.term)} ${stay.academic_year}`
      }
      footer={
        <>
          <Button variant="secondary" onClick={close} disabled={pay.busy}>
            Cancel
          </Button>
          <Button type="submit" icon="printer" loading={pay.busy}>
            Record and print receipt
          </Button>
        </>
      }
    >
      {stay && (
        <>
          <MiniStats
            items={[
              { label: 'Paid so far', value: desk.formatMoney(stay.total_paid) },
              { label: 'Balance', value: desk.formatMoney(stay.balance) },
            ]}
          />
          <Field
            label="Amount"
            error={amountError}
            hint={
              owing
                ? 'The full balance is filled in. Change it for a part payment.'
                : 'This stay is already paid up.'
            }
          >
            <div className="relative">
              <span className="pointer-events-none absolute inset-y-0 start-0 flex items-center ps-4 text-sm text-gray-500">
                {desk.currency}
              </span>
              <Input
                name="amount"
                inputMode="decimal"
                autoComplete="off"
                className="ps-14 tabular-nums"
                // Prefill the balance string as typed text; no arithmetic. Keyed by the
                // balance: a sync that changes it re-prefills rather than leave a stale amount.
                key={stay.balance}
                defaultValue={owing ? stay.balance.replace(/\.0+$/, '') : ''}
                onFocus={(e) => e.currentTarget.select()}
                onChange={() => setAmountError(null)}
              />
            </div>
          </Field>
          <fieldset>
            <legend className="text-sm font-medium text-gray-900">Method</legend>
            <div className="mt-1.5 grid grid-cols-2 gap-2">
              {METHODS.map((m) => (
                <label
                  key={m.value}
                  className="flex cursor-pointer items-center gap-3 rounded-full border border-gray-300 bg-white px-4 py-2.5 text-sm font-medium text-gray-700 shadow-xs transition-colors hover:bg-gray-50 has-checked:border-primary-900 has-checked:text-gray-900 has-checked:ring-1 has-checked:ring-primary-900"
                >
                  <input
                    type="radio"
                    name="method"
                    value={m.value}
                    defaultChecked={m.value === 'cash'}
                    className="size-4 border-gray-300 text-primary-900 focus:ring-primary-900"
                  />
                  {m.label}
                </label>
              ))}
            </div>
          </fieldset>
          <Field label="Notes" hint="Optional. Printed on the receipt.">
            <Textarea name="notes" rows={2} />
          </Field>
          {pay.error && <Alert title="The payment was not recorded">{pay.error}</Alert>}
        </>
      )}
    </Modal>
  );
}
