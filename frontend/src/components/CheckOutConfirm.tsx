import { useState } from 'react';
import { Modal } from './ui/Modal.tsx';
import { Button } from './ui/Button.tsx';
import { formatDecimal } from '../lib/decimal.ts';
import { checkOut } from '../lib/tauri.ts';
import { addToast } from '../lib/toasts.ts';

type Props = {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  hostelId: number;
  currency: string;
  allocationId: number;
  residentName: string;
  roomNumber: string;
  balance: string;
  onDone: () => void;
};

export function CheckOutConfirm({
  open,
  onOpenChange,
  hostelId,
  currency,
  allocationId,
  residentName,
  roomNumber,
  balance,
  onDone,
}: Props) {
  const [busy, setBusy] = useState(false);
  const today = new Date().toLocaleDateString('en-CA');
  const owed = BigInt(balance.replace('.', '').replace(/[^0-9]/g, '') || '0') > 0n;

  async function handleConfirm() {
    setBusy(true);
    try {
      await checkOut({
        hostel_id: hostelId,
        allocation_id: allocationId,
        check_out_date: today,
      });
      addToast(`Checked out ${residentName} from ${roomNumber}.`, 'success');
      onOpenChange(false);
      onDone();
    } catch {
      // toast already surfaced by the wrapper
    } finally {
      setBusy(false);
    }
  }

  return (
    <Modal
      open={open}
      onOpenChange={onOpenChange}
      title="Check out resident"
      description={`Check out ${residentName} from ${roomNumber} today?`}
    >
      <div className="flex flex-col gap-2">
        <p className="text-body-sm text-muted">
          Outstanding balance:{' '}
          <span className={owed ? 'text-error-text font-medium' : 'text-ink'}>
            {currency} {formatDecimal(balance)}
          </span>
        </p>
        {owed && (
          <p className="text-caption-sm text-error-text">
            This resident still owes money. Please collect payment before checkout.
          </p>
        )}
      </div>
      <div className="flex justify-end">
        <Button variant={owed ? 'secondary' : 'primary'} onClick={handleConfirm} disabled={busy}>
          {busy ? 'Checking out…' : 'Confirm check-out'}
        </Button>
      </div>
    </Modal>
  );
}
