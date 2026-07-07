import { useState } from 'react';
import {
  activateLicense,
  generateTrialLicense,
  type LicensePayload,
} from '../lib/tauri.ts';
import logo from '../assets/logo.png';
import { Button } from './ui/Button.tsx';
import { Textarea } from './ui/Textarea.tsx';

type Props = {
  onActivated: (payload: LicensePayload) => void;
};

export function LockScreen({ onActivated }: Props) {
  const [license, setLicense] = useState('');
  const [status, setStatus] = useState<'idle' | 'submitting'>('idle');
  const [error, setError] = useState<string | null>(null);

  const handleSubmit = async (event: React.FormEvent) => {
    event.preventDefault();
    setError(null);
    setStatus('submitting');

    try {
      const payload = await activateLicense(license.trim());
      onActivated(payload);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setStatus('idle');
    }
  };

  const handleStartTrial = async () => {
    setError(null);
    setStatus('submitting');

    try {
      const trial = await generateTrialLicense();
      const payload = await activateLicense(trial);
      onActivated(payload);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setStatus('idle');
    }
  };

  return (
    <div className="flex min-h-screen items-center justify-center bg-canvas p-4">
      <div className="w-full max-w-md rounded-md bg-canvas p-8 shadow-card">
        <img
          src={logo}
          alt="HostelDesk"
          className="mx-auto mb-4 h-24 w-24 rounded-lg"
        />
        <h1 className="mb-2 text-center text-display-md text-ink">
          Activate HostelDesk
        </h1>
        <p className="mb-6 text-center text-body-sm text-muted">
          Enter your license key to unlock the application.
        </p>

        <form onSubmit={handleSubmit} className="flex flex-col gap-4">
          <Textarea
            label="License key"
            value={license}
            onChange={(e) => setLicense(e.target.value)}
            rows={6}
            placeholder="Paste your license key here"
            required
            error={error ?? undefined}
          />

          <Button type="submit" disabled={status === 'submitting'} className="w-full">
            {status === 'submitting' ? 'Activating…' : 'Activate'}
          </Button>

          <div className="relative py-2">
            <div className="absolute inset-0 flex items-center">
              <div className="w-full border-t border-hairline" />
            </div>
            <div className="relative flex justify-center">
              <span className="bg-canvas px-2 text-caption-sm text-muted">or</span>
            </div>
          </div>

          <Button
            variant="secondary"
            onClick={handleStartTrial}
            disabled={status === 'submitting'}
            className="w-full"
          >
            Start 30-day trial
          </Button>
        </form>
      </div>
    </div>
  );
}
