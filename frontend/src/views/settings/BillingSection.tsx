import { useEffect, useState } from 'react';
import { Card } from '../../components/ui/Card.tsx';
import { Button } from '../../components/ui/Button.tsx';
import { Badge } from '../../components/ui/Badge.tsx';
import { Modal } from '../../components/ui/Modal.tsx';
import { Textarea } from '../../components/ui/Textarea.tsx';
import { addToast } from '../../lib/toasts.ts';
import {
  getLicenseStatus,
  activateLicense,
  type LicensePayload,
} from '../../lib/tauri.ts';

export function BillingSection() {
  const [license, setLicense] = useState<LicensePayload | null>(null);
  const [loading, setLoading] = useState(true);
  const [enterOpen, setEnterOpen] = useState(false);
  const [key, setKey] = useState('');
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    getLicenseStatus()
      .then((l) => {
        if (!cancelled) setLicense(l);
      })
      .catch(() => {})
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, []);

  async function handleActivate(e: React.FormEvent) {
    e.preventDefault();
    if (!key.trim()) return;
    setBusy(true);
    try {
      const result = await activateLicense(key.trim());
      setLicense(result);
      addToast('License activated.', 'success');
      setEnterOpen(false);
      setKey('');
    } catch {
      // toast surfaced
    } finally {
      setBusy(false);
    }
  }

  if (loading) {
    return (
      <Card shadow className="p-6">
        <div className="h-20 animate-pulse rounded-md bg-surface-soft" />
      </Card>
    );
  }

  const expiry = license ? new Date(license.expiry_date) : null;
  const daysLeft = expiry ? Math.ceil((expiry.getTime() - Date.now()) / 86_400_000) : 0;
  const expiringSoon = daysLeft < 14;

  return (
    <Card shadow className="p-6">
      <h2 className="text-title-md text-ink">License</h2>
      {license ? (
        <div className="mt-4 flex flex-col gap-2">
          <p className="text-body-sm text-ink">{license.hostel_name}</p>
          <div className="flex items-center gap-2">
            <Badge>{license.tier}</Badge>
            <span className={`text-body-sm ${expiringSoon ? 'text-error-text' : 'text-muted'}`}>
              Expires {expiry?.toLocaleDateString()} ({daysLeft} days left)
            </span>
          </div>
        </div>
      ) : (
        <div className="mt-4 flex flex-col gap-4">
          <p className="text-body-sm text-muted">
            No active license. Enter a license key to continue using HostelDesk.
          </p>
          <Button variant="secondary" onClick={() => setEnterOpen(true)} disabled={busy}>
            Enter license key
          </Button>
        </div>
      )}

      <div className="mt-6 border-t border-hairline pt-5">
        <h3 className="text-title-sm text-ink">Have a license key?</h3>
        <p className="mt-1 text-body-sm text-muted">
          Paste your key below to activate or renew your plan.
        </p>
        <div className="mt-3">
          <Button variant="secondary" size="sm" onClick={() => setEnterOpen(true)}>
            Enter new license
          </Button>
        </div>
      </div>

      <Modal
        open={enterOpen}
        onOpenChange={setEnterOpen}
        title="Enter license key"
        description="Paste your license key to activate."
      >
        <form onSubmit={handleActivate} className="flex flex-col gap-4">
          <Textarea
            label="License key"
            value={key}
            onChange={(e) => setKey(e.target.value)}
            rows={4}
          />
          <div className="flex justify-end gap-3">
            <Button variant="tertiary" onClick={() => setEnterOpen(false)} disabled={busy}>
              Cancel
            </Button>
            <Button type="submit" disabled={busy || !key.trim()}>
              Activate
            </Button>
          </div>
        </form>
      </Modal>
    </Card>
  );
}
