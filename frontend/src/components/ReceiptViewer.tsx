import { useEffect, useState } from 'react';
import { createPortal } from 'react-dom';
import * as api from '../lib/api';
import { useDesk, type ReceiptRef } from '../lib/desk';
import { pdfPageImages } from '../lib/pdf';
import { useAction } from '../lib/useLoad';
import { Alert, Button, Modal, Skeleton } from './ui';

/** A receipt's pages as images, or why they could not be shown. */
interface Loaded {
  paymentId: number;
  pages?: string[];
  error?: string;
}

/** Props of {@link ReceiptViewer}. */
export interface ReceiptViewerProps {
  /** The receipt to show; `null` keeps the viewer closed. */
  receipt: ReceiptRef | null;
  onClose: () => void;
}

/**
 * A payment's receipt inside the app: its PDF drawn by pdf.js, with Print and
 * "Open in PDF app" (the computer's own viewer, to save or email it). Print
 * sends only the receipt: its pages are copied into a print-only element
 * outside the app (see `.receipt-print` in index.css). Mounted once by
 * DeskProvider and opened with `desk.showReceipt`.
 */
export default function ReceiptViewer({ receipt, onClose }: ReceiptViewerProps) {
  const desk = useDesk();
  // Each result remembers its payment, so another receipt never shows stale pages.
  const [loaded, setLoaded] = useState<Loaded | null>(null);
  const external = useAction();

  useEffect(() => {
    if (!receipt) return;
    let live = true;
    const { paymentId } = receipt;
    api
      .receiptPdf({ hostel_id: desk.hostelId, payment_id: paymentId })
      .then(pdfPageImages)
      .then(
        (pages) => live && setLoaded({ paymentId, pages }),
        (e: unknown) => live && setLoaded({ paymentId, error: api.errorMessage(e) }),
      );
    return () => {
      live = false;
    };
  }, [receipt, desk.hostelId]);
  const current = receipt && loaded?.paymentId === receipt.paymentId ? loaded : null;
  const pages = current?.pages ?? null;
  const error = current?.error ?? null;

  const title = receipt?.receiptNumber ? `Receipt ${receipt.receiptNumber}` : 'Receipt';
  const openExternally = () => {
    if (!receipt) return;
    void external.run(() => api.openReceipt({ hostel_id: desk.hostelId, payment_id: receipt.paymentId }));
  };

  return (
    <Modal
      open={receipt !== null}
      onClose={onClose}
      size="lg"
      title={title}
      footer={
        <>
          <Button variant="secondary" loading={external.busy} onClick={openExternally}>
            Open in PDF app
          </Button>
          <Button icon="printer" disabled={!pages} onClick={() => window.print()}>
            Print
          </Button>
        </>
      }
    >
      {external.error && <Alert title="Could not open the PDF app">{external.error}</Alert>}
      {error ? (
        <Alert title="Could not show the receipt">{error}</Alert>
      ) : pages ? (
        <div className="space-y-4">
          {pages.map((src, index) => (
            <img
              key={index}
              src={src}
              alt={index === 0 ? title : `${title}, page ${index + 1}`}
              className="w-full rounded-lg border border-gray-200 shadow-xs"
            />
          ))}
        </div>
      ) : (
        <Skeleton rows={8} />
      )}
      {pages &&
        createPortal(
          <div className="receipt-print" aria-hidden="true">
            {pages.map((src, index) => (
              <img key={index} src={src} alt="" />
            ))}
          </div>,
          document.body,
        )}
    </Modal>
  );
}
