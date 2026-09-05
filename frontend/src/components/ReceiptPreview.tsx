import { convertFileSrc } from '@tauri-apps/api/core';
import { RoomsIcon } from './ui/icons.tsx';

type Props = {
  hostelName: string;
  hostelAddress?: string | null;
  hostelPhone?: string | null;
  logoPath?: string | null;
  footerText?: string | null;
};

export function ReceiptPreview({ hostelName, hostelAddress, hostelPhone, logoPath, footerText }: Props) {
  return (
    <div className="border border-hairline bg-canvas p-6 text-ink shadow-sm">
      <div className="flex items-start gap-3 border-b border-hairline pb-3">
        <div className="flex h-14 w-14 shrink-0 items-center justify-center rounded-md border border-hairline bg-surface-soft">
          {logoPath ? (
            <img src={convertFileSrc(logoPath)} alt="Hostel logo" className="h-full w-full object-contain" />
          ) : (
            <RoomsIcon width={28} height={28} className="text-muted-soft" />
          )}
        </div>
        <div>
          <p className="text-title-sm font-bold text-ink">{hostelName}</p>
          {hostelAddress && <p className="text-caption-sm text-muted">{hostelAddress}</p>}
          {hostelPhone && <p className="text-caption-sm text-muted">Tel: {hostelPhone}</p>}
        </div>
      </div>

      <div className="mt-3 flex items-center justify-between text-caption-sm">
        <span className="font-semibold">No. 000001</span>
        <span className="rounded-sm border border-hairline px-3 py-1 font-semibold">PAYMENT RECEIPT</span>
        <span>Date: {new Date().toLocaleDateString('en-GB')}</span>
      </div>

      <dl className="mt-4 flex flex-col gap-2 text-body-sm">
        <div>
          <dt className="inline text-muted">Received with thanks from: </dt>
          <dd className="inline text-ink">Jane Sample</dd>
        </div>
        <div>
          <dt className="inline text-muted">The Sum of Shillings: </dt>
          <dd className="inline text-ink">ONE HUNDRED FIFTY THOUSAND ONLY</dd>
        </div>
        <div>
          <dt className="inline text-muted">Being payment for: </dt>
          <dd className="inline text-ink">Room A01 - Rent payment</dd>
        </div>
        <div className="flex justify-between">
          <span><span className="text-muted">Payment method: </span>CASH</span>
          <span><span className="text-muted">Balance: </span>UGX 0</span>
        </div>
      </dl>

      <div className="mt-4 flex items-center justify-between">
        <div className="border border-hairline px-4 py-2 text-body-sm font-bold">SHS. 150,000</div>
        <span className="text-caption-sm text-muted">With thanks.</span>
      </div>

      <p className="mt-4 text-caption-sm text-muted">
        {footerText?.trim() || 'The amount paid is non-refundable.'}
      </p>

      <div className="mt-8 flex justify-end">
        <div className="text-right text-caption-sm">
          <p className="border-t border-hairline pt-1">Sign</p>
          <p className="mt-1 text-muted">For: {hostelName}</p>
        </div>
      </div>
    </div>
  );
}
