import { useState, type FormEvent } from 'react';
import { Alert, Button, Card, Field, Input } from '../../components/ui';
import * as api from '../../lib/api';
import { useDesk } from '../../lib/desk';
import { looksLikeEmail } from '../../lib/format';
import type { User } from '../../lib/types';
import { useAction } from '../../lib/useLoad';
import Section from './Section';

/** Settings → Account: your name and email, and a separate password change. Everyone sees it. */
export default function AccountSection() {
  const desk = useDesk();
  // ponytail: desk.user is the sign-in snapshot; keep the last saved copy here so both forms send current values.
  const [me, setMe] = useState<User>(desk.user);
  return (
    <>
      <Section title="Profile" description="Your name and the email you sign in with.">
        <ProfileForm user={me} onSaved={setMe} />
      </Section>
      <Section
        title="Password"
        description="Change the password you sign in with. You need your current password to do it."
      >
        <PasswordForm user={me} />
      </Section>
    </>
  );
}

function ProfileForm({ user, onSaved }: { user: User; onSaved: (user: User) => void }) {
  const desk = useDesk();
  const [name, setName] = useState(user.full_name);
  const [email, setEmail] = useState(user.email);
  const [errors, setErrors] = useState<{ name?: string; email?: string }>({});
  const save = useAction();
  const dirty = name.trim() !== user.full_name || email.trim() !== user.email;

  const submit = (event: FormEvent) => {
    event.preventDefault();
    if (!dirty) return;
    const found: typeof errors = {};
    if (!name.trim()) found.name = 'Enter your name.';
    if (!looksLikeEmail(email)) found.email = 'Enter a valid email address.';
    setErrors(found);
    if (Object.keys(found).length > 0) return;
    void save.run(async () => {
      const updated = await api.updateUser({
        user_id: user.id,
        full_name: name.trim(),
        email: email.trim(),
        role: user.role,
        password: null,
        current_password: null,
      });
      onSaved(updated);
      desk.toast({ tone: 'success', title: 'Account saved.' });
      desk.reload();
    });
  };

  return (
    <form onSubmit={submit} noValidate>
      <Card
        footer={
          <Button type="submit" disabled={!dirty} loading={save.busy}>
            Save
          </Button>
        }
      >
        <div className="grid gap-5">
          <Field label="Full name" error={errors.name}>
            <Input autoComplete="name" value={name} onChange={(e) => setName(e.target.value)} />
          </Field>
          <Field label="Email" error={errors.email}>
            <Input
              type="email"
              autoComplete="email"
              value={email}
              onChange={(e) => setEmail(e.target.value)}
            />
          </Field>
        </div>
        {save.error && (
          <div className="mt-5">
            <Alert title="Could not save your account">{save.error}</Alert>
          </div>
        )}
      </Card>
    </form>
  );
}

const EMPTY = { current: '', next: '', confirm: '' };
type PasswordKey = keyof typeof EMPTY;
const WRONG_PASSWORD = 'Current password is incorrect.';

function PasswordForm({ user }: { user: User }) {
  const desk = useDesk();
  const [values, setValues] = useState(EMPTY);
  const [errors, setErrors] = useState<Partial<Record<PasswordKey, string>>>({});
  const save = useAction();
  const typed = Object.values(values).some(Boolean);
  const set = (key: PasswordKey) => (e: { target: { value: string } }) =>
    setValues((v) => ({ ...v, [key]: e.target.value }));

  const submit = (event: FormEvent) => {
    event.preventDefault();
    const found: typeof errors = {};
    if (!values.current) found.current = 'Enter your current password.';
    if ([...values.next].length < 8) found.next = 'Use at least 8 characters.';
    else if (values.next !== values.confirm) found.confirm = 'The new passwords do not match.';
    setErrors(found);
    if (Object.keys(found).length > 0) return;
    void save.run(async () => {
      try {
        await api.updateUser({
          user_id: user.id,
          full_name: user.full_name,
          email: user.email,
          role: user.role,
          password: values.next,
          current_password: values.current,
        });
      } catch (error) {
        if (api.errorMessage(error) !== WRONG_PASSWORD) throw error;
        setErrors({ current: WRONG_PASSWORD });
        return;
      }
      setValues(EMPTY);
      desk.toast({ tone: 'success', title: 'Password changed.' });
    });
  };

  return (
    <form onSubmit={submit} noValidate>
      <Card
        footer={
          <Button type="submit" disabled={!typed} loading={save.busy}>
            Change password
          </Button>
        }
      >
        <div className="grid gap-5">
          <Field label="Current password" error={errors.current}>
            <Input
              type="password"
              autoComplete="current-password"
              value={values.current}
              onChange={set('current')}
            />
          </Field>
          <Field label="New password" hint="At least 8 characters." error={errors.next}>
            <Input type="password" autoComplete="new-password" value={values.next} onChange={set('next')} />
          </Field>
          <Field label="Repeat new password" error={errors.confirm}>
            <Input
              type="password"
              autoComplete="new-password"
              value={values.confirm}
              onChange={set('confirm')}
            />
          </Field>
        </div>
        {save.error && (
          <div className="mt-5">
            <Alert title="Could not change your password">{save.error}</Alert>
          </div>
        )}
      </Card>
    </form>
  );
}
