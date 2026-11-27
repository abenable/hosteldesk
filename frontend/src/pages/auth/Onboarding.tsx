import { useState, type FormEvent } from 'react';
import { Alert, Button, Card, Field, Input, Steps } from '../../components/ui';
import * as api from '../../lib/api';
import { blank, looksLikeEmail } from '../../lib/format';
import { toast } from '../../lib/toast';
import type { BootState } from '../../lib/types';
import { useAction } from '../../lib/useLoad';
import { RoomSetup } from '../rooms/RoomSetup';
import AuthLayout from './AuthLayout';

type Key = 'name' | 'address' | 'phone' | 'admin' | 'email' | 'password';

/** First-run steps for a new hostel. The rooms steps run once the administrator is signed in. */
const STEPS = ['Hostel', 'Administrator', 'Room classes', 'Rooms'];
/** Steps for a hostel pulled from the cloud without any room classes yet. */
const JOIN_STEPS = ['Administrator', 'Room classes', 'Rooms'];

/** The currency a new hostel starts with; Settings → Business changes it. */
const CURRENCY = 'UGX';

/**
 * First run, as a wizard. Two modes (`boot.hostel_exists`):
 * - new hostel: hostel details → first administrator (saved together, which
 *   signs the administrator in) → room classes → rooms;
 * - hostel already pulled from the cloud: the first administrator, then the
 *   rooms steps only when no room class came with it.
 * When it finishes the gate re-boots into the app. Leaving during the rooms
 * steps is fine: the Rooms page reopens the same wizard.
 */
export default function Onboarding({ boot, reboot }: { boot: BootState; reboot: () => Promise<void> }) {
  const existing = boot.hostel_exists;
  const [step, setStep] = useState(existing ? 1 : 0);
  const [hostelId, setHostelId] = useState<number | null>(null);
  const [values, setValues] = useState<Record<Key, string>>({
    name: '',
    address: '',
    phone: '',
    admin: '',
    email: '',
    password: '',
  });
  const [errors, setErrors] = useState<Partial<Record<Key, string>>>({});
  const save = useAction();
  const set = (key: Key) => (e: { target: { value: string } }) =>
    setValues((v) => ({ ...v, [key]: e.target.value }));

  const nextFromHostel = (event: FormEvent) => {
    event.preventDefault();
    const found: Partial<Record<Key, string>> = {};
    if (!values.name.trim()) found.name = 'Enter the hostel name.';
    setErrors(found);
    if (Object.keys(found).length === 0) setStep(1);
  };

  const createAdmin = (event: FormEvent) => {
    event.preventDefault();
    const found: Partial<Record<Key, string>> = {};
    if (!values.admin.trim()) found.admin = 'Enter your full name.';
    if (!looksLikeEmail(values.email)) found.email = 'Enter a valid email address.';
    if (values.password.length < 8) found.password = 'Use at least 8 characters.';
    setErrors(found);
    if (Object.keys(found).length > 0) return;

    void save.run(async () => {
      const user = await api.completeOnboarding({
        // An existing hostel (received from the cloud) keeps its name; Rust skips the check.
        hostel_name: existing ? '' : values.name.trim(),
        address: existing ? null : blank(values.address),
        phone: existing ? null : blank(values.phone),
        currency: CURRENCY,
        branding_logo_path: null,
        admin_name: values.admin.trim(),
        email: values.email.trim(),
        password: values.password,
      });
      if (existing && (await api.roomClasses(user.hostel_id)).length > 0) {
        toast({ tone: 'success', title: 'Administrator created.' });
        await reboot();
        return;
      }
      toast({
        tone: 'success',
        title: existing ? 'Administrator created.' : 'Your hostel is set up.',
        message: 'Now add its rooms.',
      });
      setHostelId(user.hostel_id);
      setStep(2);
    });
  };

  const finish = () => {
    toast({ tone: 'success', title: 'Your hostel is ready.' });
    void reboot();
  };

  // Steps 2+ are the room setup, which moves between classes and rooms itself.
  const [title, description] =
    step >= 2
      ? [
          'Add your rooms',
          'Group your rooms into classes by floor, type and price, then add the rooms of each class.',
        ]
      : existing
        ? [
            'Create the first administrator',
            'This computer received its hostel from the cloud. Create the account you will sign in with here.',
          ]
        : step === 0
          ? ['Set up your hostel', 'Tell HostelDesk about your hostel.']
          : [
              'Create your administrator account',
              'You sign in with this account. You can add more staff later in Settings.',
            ];

  return (
    <AuthLayout wide title={title} description={description} footer={`HostelDesk ${boot.app_version}`}>
      {hostelId !== null && step >= 2 ? (
        <RoomSetup
          hostelId={hostelId}
          currency={CURRENCY}
          steps={existing ? JOIN_STEPS : STEPS}
          doneLabel="Open HostelDesk"
          onDone={finish}
        />
      ) : step === 0 ? (
        <form onSubmit={nextFromHostel} noValidate className="space-y-6">
          <Steps labels={STEPS} current={0} />
          <Card footer={<Button type="submit">Next: administrator</Button>}>
            <div className="space-y-4">
              <Field label="Hostel name" error={errors.name}>
                <Input autoComplete="organization" value={values.name} onChange={set('name')} />
              </Field>
              <div className="grid grid-cols-1 gap-4 sm:grid-cols-2">
                <Field label="Address" hint="Optional. Printed on receipts.">
                  <Input autoComplete="street-address" value={values.address} onChange={set('address')} />
                </Field>
                <Field label="Phone" hint="Optional. Printed on receipts.">
                  <Input type="tel" autoComplete="tel" value={values.phone} onChange={set('phone')} />
                </Field>
              </div>
            </div>
          </Card>
        </form>
      ) : (
        <form onSubmit={createAdmin} noValidate className="space-y-6">
          {!existing && <Steps labels={STEPS} current={1} />}
          <Card
            footer={
              <div className="flex w-full items-center justify-between gap-3">
                {existing ? (
                  <span />
                ) : (
                  <Button variant="secondary" icon="arrow-left" onClick={() => setStep(0)} disabled={save.busy}>
                    Hostel
                  </Button>
                )}
                <Button type="submit" loading={save.busy}>
                  {existing ? 'Create administrator' : 'Next: room classes'}
                </Button>
              </div>
            }
          >
            <div className="space-y-4">
              <Field label="Full name" error={errors.admin}>
                <Input autoComplete="name" value={values.admin} onChange={set('admin')} />
              </Field>
              <div className="grid grid-cols-1 gap-4 sm:grid-cols-2">
                <Field label="Email" error={errors.email}>
                  <Input type="email" autoComplete="email" value={values.email} onChange={set('email')} />
                </Field>
                <Field label="Password" hint="At least 8 characters." error={errors.password}>
                  <Input
                    type="password"
                    autoComplete="new-password"
                    value={values.password}
                    onChange={set('password')}
                  />
                </Field>
              </div>
              {save.error && <Alert title="Could not finish setup">{save.error}</Alert>}
            </div>
          </Card>
        </form>
      )}
    </AuthLayout>
  );
}
