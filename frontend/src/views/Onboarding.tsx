import { useState } from 'react';
import { Button } from '../components/ui/Button.tsx';
import { Input } from '../components/ui/Input.tsx';
import { Select } from '../components/ui/Select.tsx';
import { Card } from '../components/ui/Card.tsx';
import { Stepper } from '../components/ui/Stepper.tsx';
import { addToast } from '../lib/toasts.ts';
import logo from '../assets/logo.png';
import {
  createStaffUser,
  createHostel,
  listHostels,
  activateLicense,
  generateTrialLicense,
} from '../lib/tauri.ts';
import {
  saveHostelProfile,
  saveAdminAccount,
  completeOnboarding,
  type HostelProfile,
  type StaffRole,
} from '../lib/onboarding.ts';

const STEPS = [
  { id: 1, label: 'Hostel profile' },
  { id: 2, label: 'Admin account' },
  { id: 3, label: 'Review & finish' },
];

const ROLES: { value: StaffRole; label: string }[] = [
  { value: 'admin', label: 'Admin' },
  { value: 'manager', label: 'Manager' },
];

type LicenseChoice = 'trial' | 'key';

async function ensureHostel(profile: HostelProfile): Promise<number> {
  const existing = await listHostels();
  if (existing.length > 0) return existing[0].id;
  const hostel = await createHostel({
    name: profile.name.trim(),
    address: profile.address?.trim() || undefined,
    phone: profile.phone?.trim() || undefined,
  });
  return hostel.id;
}

export function Onboarding() {
  const [phase, setPhase] = useState<'welcome' | 'wizard'>('welcome');
  if (phase === 'welcome') {
    return <Welcome onStart={() => setPhase('wizard')} />;
  }
  return <Wizard />;
}

function Welcome({ onStart }: { onStart: () => void }) {
  return (
    <div className="flex min-h-screen flex-col items-center justify-center bg-canvas px-4 py-16 text-center">
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
  const [adminRole, setAdminRole] = useState<StaffRole>('admin');
  const [creating, setCreating] = useState(false);

  // Step 3 — license
  const [licenseChoice, setLicenseChoice] = useState<LicenseChoice>('trial');
  const [licenseKey, setLicenseKey] = useState('');
  const [finishing, setFinishing] = useState(false);

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
      saveHostelProfile(profile);
      setStep(2);
      return;
    }
    if (step === 2) {
      void handleCreateAdmin();
      return;
    }
    if (step === 3) {
      void handleFinish();
    }
  }

  function back() {
    if (step > 1) setStep(step - 1);
  }

  async function handleCreateAdmin() {
    if (!adminValid) {
      addToast(
        'Fill all fields. Use a valid email, a phone number, and a password of 8+ characters.',
        'error',
      );
      return;
    }
    setCreating(true);
    try {
      const hostelId = await ensureHostel(profile);
      await createStaffUser({
        hostel_id: hostelId,
        name: adminName.trim(),
        username: adminEmail.trim(),
        password: adminPassword,
        role: adminRole,
      });
      saveAdminAccount({
        name: adminName.trim(),
        email: adminEmail.trim(),
        phone: adminPhone.trim(),
        role: adminRole,
      });
      addToast('Account created.', 'success');
      setStep(3);
    } catch {
      // toast already surfaced by the tauri wrapper
    } finally {
      setCreating(false);
    }
  }

  async function handleFinish() {
    setFinishing(true);
    try {
      if (licenseChoice === 'key') {
        if (!licenseKey.trim()) {
          addToast('Paste your license key to continue.', 'error');
          setFinishing(false);
          return;
        }
        await activateLicense(licenseKey.trim());
      } else {
        const trial = await generateTrialLicense();
        await activateLicense(trial);
      }
      completeOnboarding();
      addToast('Onboarding complete. Welcome to HostelDesk.', 'success');
    } catch {
      // toast already surfaced by the tauri wrapper
    } finally {
      setFinishing(false);
    }
  }

  return (
    <div className="min-h-screen bg-canvas px-4 py-16">
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
                label="Email address"
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
              <Select
                label="Role"
                value={adminRole}
                capitalize
                onChange={(e) => setAdminRole(e.target.value as StaffRole)}
              >
                {ROLES.map((r) => (
                  <option key={r.value} value={r.value}>
                    {r.label}
                  </option>
                ))}
              </Select>
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
                  <dd className="text-ink capitalize">{adminRole}</dd>
                </div>
              </dl>

              <div className="border-t border-hairline pt-5">
                <h3 className="text-title-sm text-ink">License</h3>
                <p className="mt-1 text-caption-sm text-muted">
                  Start a 30-day free trial, or enter a license key if you have one.
                </p>
                <div className="mt-3 inline-flex rounded-full bg-surface-soft p-1">
                  {([
                    { id: 'trial', label: 'Free trial' },
                    { id: 'key', label: 'I have a key' },
                  ] as const).map((o) => (
                    <button
                      key={o.id}
                      type="button"
                      onClick={() => setLicenseChoice(o.id)}
                      className={`rounded-full px-4 py-1.5 text-button-sm transition-colors ${
                        licenseChoice === o.id
                          ? 'bg-canvas text-ink shadow-card'
                          : 'text-muted hover:text-ink'
                      }`}
                    >
                      {o.label}
                    </button>
                  ))}
                </div>
                {licenseChoice === 'key' && (
                  <Input
                    className="mt-3"
                    label="License key"
                    value={licenseKey}
                    onChange={(e) => setLicenseKey(e.target.value)}
                    placeholder="Paste your license key"
                  />
                )}
              </div>
            </div>
          )}

          <div
            className={`mt-6 flex items-center ${
              step > 1 ? 'justify-between' : 'justify-center'
            }`}
          >
            {step > 1 && (
              <Button variant="tertiary" onClick={back} disabled={creating || finishing}>
                Back
              </Button>
            )}
            <Button
              onClick={next}
              disabled={
                (step === 1 && !profileValid) ||
                (step === 2 && (!adminValid || creating)) ||
                creating ||
                finishing
              }
            >
              {creating || finishing
                ? 'Working…'
                : step === 3
                  ? 'Finish'
                  : step === 2
                    ? 'Create account & continue'
                    : 'Continue'}
            </Button>
          </div>
        </Card>
      </div>
    </div>
  );
}
