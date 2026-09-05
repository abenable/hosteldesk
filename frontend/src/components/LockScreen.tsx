import { useState } from 'react';
import { activateLicense, type LicensePayload } from '../lib/tauri.ts';
import logo from '../assets/logo.png';
import { Button } from './ui/Button.tsx';
import { Card } from './ui/Card.tsx';
import { Input } from './ui/Input.tsx';

type Props = {
  onActivated: (payload: LicensePayload) => void;
};

export function LockScreen({ onActivated }: Props) {
  const [licenseKey, setLicenseKey] = useState('');
  const [status, setStatus] = useState<'idle' | 'submitting'>('idle');
  const [error, setError] = useState<string | null>(null);

  const handleSubmit = async (event: React.FormEvent) => {
    event.preventDefault();
    setError(null);

    if (!licenseKey.trim()) {
      setError('Paste a license key to continue.');
      return;
    }

    setStatus('submitting');

    try {
      const payload = await activateLicense(licenseKey.trim());
      onActivated(payload);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
      setStatus('idle');
    }
  };

  return (
    <div className="flex h-screen items-center justify-center overflow-y-auto bg-canvas p-4">
      <Card
        shadow
        className="w-full max-w-xl animate-[lock-screen-in_0.3s_ease-out] p-8 text-center"
      >
        <img
          src={logo}
          alt="HostelDesk"
          className="mx-auto mb-6 h-20 w-20 rounded-lg ring-1 ring-ink/10"
        />
        <h1 className="text-display-md text-ink">Activate HostelDesk</h1>
        <p className="mt-2 text-body-sm text-muted">
          Paste the license key you received to unlock the app.
        </p>

        <form onSubmit={handleSubmit} className="mt-8 flex flex-col gap-4 text-left">
          <Input
            label="License key"
            value={licenseKey}
            onChange={(e) => {
              setLicenseKey(e.target.value);
              if (error) setError(null);
            }}
            placeholder="Paste your license key here"
            error={error ?? undefined}
            autoFocus
          />
          <Button
            type="submit"
            disabled={status === 'submitting' || !licenseKey.trim()}
            className="transition-transform active:scale-[0.96]"
          >
            {status === 'submitting' ? 'Activating…' : 'Activate'}
          </Button>
        </form>
      </Card>
    </div>
  );
}
