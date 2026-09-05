import { useState } from 'react';
import { Button } from '../components/ui/Button.tsx';
import { Input } from '../components/ui/Input.tsx';
import { Card } from '../components/ui/Card.tsx';
import { Stepper } from '../components/ui/Stepper.tsx';
import { addToast } from '../lib/toasts.ts';
import logo from '../assets/logo.png';
import { completeOnboarding as completeOnboardingCommand } from '../lib/tauri.ts';
import { completeOnboarding } from '../lib/onboarding.ts';
import { setCurrentUser } from '../lib/current-user.ts';

type HostelProfile = {
  name: string;
  address?: string;
  phone?: string;
};

const STEPS = [
  { id: 1, label: 'Hostel profile' },
  { id: 2, label: 'Admin account' },
  { id: 3, label: 'Review & finish' },
];

export function Onboarding() {
  const [phase, setPhase] = useState<'welcome' | 'wizard'>('welcome');
  if (phase === 'welcome') {
    return <Welcome onStart={() => setPhase('wizard')} />;
  }
  return <Wizard />;
}

function Welcome({ onStart }: { onStart: () => void }) {
  return (
    <div className="flex h-screen flex-col items-center justify-center overflow-y-auto bg-canvas px-4 py-16 text-center">
      <div className="mx-auto max-w-md">
        <img
          src={logo}
          alt="HostelDesk"
          className="mx-auto mb-6 h-28 w-28 rounded-lg"
        />
        <h1 className="text-display-xl text-ink">Welcome to HostelDesk</h1>
        <p className="mt-3 text-body-md text-muted">
          Check residents in, track rooms, record payments, and generate
          receipts — all in a few quick steps. Let's set up your hostel.
        </p>
        <div className="mt-8 flex justify-center">
          <Button onClick={onStart}>Get started</Button>
        </div>
      </div>
    </div>
  );
}

function Wizard() {
  const [step, setStep] = useState(1);

  // Step 1
  const [profile, setProfile] = useState<HostelProfile>({
    name: '',
    address: '',
    phone: '',
  });

  // Step 2
  const [adminName, setAdminName] = useState('');
  const [adminEmail, setAdminEmail] = useState('');
  const [adminPhone, setAdminPhone] = useState('');
  const [adminPassword, setAdminPassword] = useState('');
  const [creating, setCreating] = useState(false);

  const profileValid = profile.name.trim().length > 0;
  const emailValid = /^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(adminEmail.trim());
  const adminValid =
    adminName.trim().length > 0 &&
    emailValid &&
    adminPhone.trim().length > 0 &&
    adminPassword.length >= 8;

  function next() {
    if (step === 1) {
      if (!profileValid) {
        addToast('Hostel name is required.', 'error');
        return;
      }
      setStep(2);
      return;
    }
    if (step === 2) {
      setStep(3);
      return;
    }
    if (step === 3) {
      void handleFinish();
    }
  }

  function back() {
    if (step > 1) setStep(step - 1);
  }

  async function handleFinish() {
    if (!adminValid) {
      addToast(
        'Fill all fields. Use a valid email, a phone number, and a password of 8+ characters.',
        'error',
      );
      return;
    }
    setCreating(true);
    try {
      const user = await completeOnboardingCommand({
        hostel_name: profile.name.trim(),
        address: profile.address?.trim() || null,
        phone: profile.phone?.trim() || null,
        currency: null,
        branding_logo_path: null,
        admin_name: adminName.trim(),
        email: adminEmail.trim(),
        password: adminPassword,
      });
      setCurrentUser(user);
      completeOnboarding(user);
    } catch {
      // The Tauri wrapper already surfaced the error.
    } finally {
      setCreating(false);
    }
  }

  return (
    <div className="h-screen overflow-y-auto bg-canvas px-4 py-16">
      <div className="mx-auto max-w-xl text-center">
        <header className="mb-8">
          <h1 className="text-display-lg text-ink">Set up your hostel</h1>
          <p className="mt-1 text-body-sm text-muted">
            A few quick steps and you are ready to check in your first resident.
          </p>
        </header>

        <div className="mb-8">
          <Stepper steps={STEPS} current={step} />
        </div>

        <Card shadow className="p-6 text-left">
          {step === 1 && (
            <div className="flex flex-col gap-4">
              <h2 className="text-display-sm text-center text-ink">Hostel profile</h2>
              <Input
                label="Hostel name"
                value={profile.name}
                onChange={(e) => setProfile({ ...profile, name: e.target.value })}
                placeholder="Sunrise Backpackers"
                required
              />
              <Input
                label="Address"
                value={profile.address}
                onChange={(e) => setProfile({ ...profile, address: e.target.value })}
                placeholder="123 Kampala Road"
              />
              <Input
                label="Phone"
                type="tel"
                value={profile.phone}
                onChange={(e) => setProfile({ ...profile, phone: e.target.value })}
                placeholder="+256 700 000 000"
              />
            </div>
          )}

          {step === 2 && (
            <div className="flex flex-col gap-4">
              <h2 className="text-display-sm text-center text-ink">Admin account</h2>
              <p className="text-center text-body-sm text-muted">
                This account can sign in and manage the hostel.
              </p>
              <Input
                label="Name"
                value={adminName}
                onChange={(e) => setAdminName(e.target.value)}
                placeholder="Jane Doe"
              />
              <Input
                label="Email"
                type="email"
                value={adminEmail}
                onChange={(e) => setAdminEmail(e.target.value)}
                placeholder="jane@hostel.com"
                error={
                  adminEmail.length > 0 && !emailValid
                    ? 'Enter a valid email address.'
                    : undefined
                }
              />
              <Input
                label="Phone"
                type="tel"
                value={adminPhone}
                onChange={(e) => setAdminPhone(e.target.value)}
                placeholder="+256 700 000 000"
              />
              <Input
                label="Password"
                type="password"
                value={adminPassword}
                onChange={(e) => setAdminPassword(e.target.value)}
                hint="At least 8 characters."
              />
            </div>
          )}

          {step === 3 && (
            <div className="flex flex-col gap-5">
              <h2 className="text-display-sm text-center text-ink">Review & finish</h2>
              <dl className="flex flex-col gap-3 text-body-md">
                <div>
                  <dt className="text-caption text-muted">Hostel</dt>
                  <dd className="text-ink">{profile.name || '—'}</dd>
                </div>
                <div>
                  <dt className="text-caption text-muted">Name</dt>
                  <dd className="text-ink">{adminName || '—'}</dd>
                </div>
                <div>
                  <dt className="text-caption text-muted">Email</dt>
                  <dd className="text-ink">{adminEmail || '—'}</dd>
                </div>
                <div>
                  <dt className="text-caption text-muted">Role</dt>
                  <dd className="text-ink">Administrator</dd>
                </div>
              </dl>

              <p className="text-center text-body-sm text-muted">
                Everything looks good. Click Finish to start using HostelDesk.
              </p>
            </div>
          )}

          <div
            className={`mt-6 flex items-center ${
              step > 1 ? 'justify-between' : 'justify-center'
            }`}
          >
            {step > 1 && (
              <Button variant="tertiary" onClick={back} disabled={creating}>
                Back
              </Button>
            )}
            <Button
              onClick={next}
              disabled={
                (step === 1 && !profileValid) ||
                (step === 2 && (!adminValid || creating)) ||
                creating
              }
            >
              {creating
                ? 'Working…'
                : step === 3
                  ? 'Finish'
                  : step === 2
                    ? 'Continue'
                    : 'Continue'}
            </Button>
          </div>
        </Card>
      </div>
    </div>
  );
}
