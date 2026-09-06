import { useState } from 'react';
import { activateLicenseFromFile, type LicensePayload } from '../lib/tauri.ts';
import logo from '../assets/logo.png';
import { Button } from './ui/Button.tsx';
import { Card } from './ui/Card.tsx';

type Props = {
  onActivated: (payload: LicensePayload) => void;
};

export function LockScreen({ onActivated }: Props) {
  const [status, setStatus] = useState<'idle' | 'submitting'>('idle');
  const [error, setError] = useState<string | null>(null);

  const handleChooseFile = async () => {
    setError(null);
    setStatus('submitting');

    try {
      const payload = await activateLicenseFromFile();
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
          Select the license file you received to unlock the app.
        </p>

        <div className="mt-8 flex flex-col gap-4">
          <Button
            type="button"
            onClick={handleChooseFile}
            disabled={status === 'submitting'}
            className="transition-transform active:scale-[0.96]"
          >
            {status === 'submitting' ? 'Activating…' : 'Choose license file…'}
          </Button>
          {error && <p className="text-caption-sm text-error-text">{error}</p>}
        </div>
      </Card>
    </div>
  );
}
