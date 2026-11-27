import { useState, type FormEvent } from 'react';
import { Alert, Button, Card, Field, Input } from '../../components/ui';
import * as api from '../../lib/api';
import { blank, looksLikeEmail } from '../../lib/format';
import { toast } from '../../lib/toast';
import type { BootState } from '../../lib/types';
import { useAction } from '../../lib/useLoad';
import AuthLayout from './AuthLayout';

type Key = 'name' | 'address' | 'phone' | 'admin' | 'email' | 'password';

/**
 * First run. Two modes (`boot.hostel_exists`):
 * - new hostel: hostel name/address/phone + the first administrator;
 * - hostel already pulled from the cloud: only the first administrator.
 * On success the admin is signed in and the gate re-boots into the app.
 */
export default function Onboarding({ boot, reboot }: { boot: BootState; reboot: () => Promise<void> }) {
  const existing = boot.hostel_exists;
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

  const submit = (event: FormEvent) => {
    event.preventDefault();
    const found: Partial<Record<Key, string>> = {};
    if (!existing && !values.name.trim()) found.name = 'Enter the hostel name.';
    if (!values.admin.trim()) found.admin = 'Enter your full name.';
    if (!looksLikeEmail(values.email)) found.email = 'Enter a valid email address.';
    if (values.password.length < 8) found.password = 'Use at least 8 characters.';
    setErrors(found);
    if (Object.keys(found).length > 0) return;

    void save.run(async () => {
      await api.completeOnboarding({
        // An existing hostel (received from the cloud) keeps its name; Rust skips the check.
        hostel_name: existing ? '' : values.name.trim(),
        address: existing ? null : blank(values.address),
        phone: existing ? null : blank(values.phone),
        currency: 'UGX',
        branding_logo_path: null,
        admin_name: values.admin.trim(),
        email: values.email.trim(),
        password: values.password,
      });
      toast({ tone: 'success', title: 'Your hostel is ready.' });
      await reboot();
    });
  };

  return (
    <AuthLayout
      wide
      title={existing ? 'Create the first administrator' : 'Set up your hostel'}
      description={
        existing
          ? 'This computer received its hostel from the cloud. Create the account you will sign in with here.'
          : 'Tell HostelDesk about your hostel and create the administrator account.'
      }
      footer={`HostelDesk ${boot.app_version}`}
    >
      <form onSubmit={submit} noValidate>
        <Card
          footer={
            <Button type="submit" loading={save.busy}>
              {existing ? 'Create administrator' : 'Set up hostel'}
            </Button>
          }
        >
          <div className="space-y-8">
            {!existing && (
              <fieldset className="space-y-4">
                <legend className="text-base font-semibold text-gray-900">Hostel</legend>
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
              </fieldset>
            )}
            <fieldset className="space-y-4">
              <legend className="text-base font-semibold text-gray-900">
                First administrator
              </legend>
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
            </fieldset>
            {save.error && <Alert title="Could not finish setup">{save.error}</Alert>}
          </div>
        </Card>
      </form>
    </AuthLayout>
  );
}
