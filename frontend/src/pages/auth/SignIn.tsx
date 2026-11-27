import { useState, type FormEvent } from 'react';
import { Alert, Button, Card, Field, Input } from '../../components/ui';
import * as api from '../../lib/api';
import { looksLikeEmail } from '../../lib/format';
import { toast } from '../../lib/toast';
import type { BootState } from '../../lib/types';
import { useAction } from '../../lib/useLoad';
import AuthLayout from './AuthLayout';

/** Staff sign-in. Enter in either field submits; the gate re-boots into the app on success. */
export default function SignIn({ boot, reboot }: { boot: BootState; reboot: () => Promise<void> }) {
  const [email, setEmail] = useState('');
  const [password, setPassword] = useState('');
  const [errors, setErrors] = useState<{ email?: string; password?: string }>({});
  const signIn = useAction();

  const submit = (event: FormEvent) => {
    event.preventDefault();
    const found = {
      email: looksLikeEmail(email) ? undefined : 'Enter your email address.',
      password: password ? undefined : 'Enter your password.',
    };
    setErrors(found);
    if (found.email || found.password) return;

    void signIn.run(async () => {
      const user = await api.login({ email: email.trim(), password });
      setPassword('');
      toast({ tone: 'success', title: `Welcome, ${user.full_name}.` });
      await reboot();
    });
  };

  return (
    <AuthLayout
      title="Sign in"
      description={boot.license ? `Staff account for ${boot.license.account_name}.` : 'Use your staff account.'}
      footer={`HostelDesk ${boot.app_version}`}
    >
      <Card>
        <form onSubmit={submit} noValidate className="space-y-5">
          <Field label="Email" error={errors.email}>
            <Input
              type="email"
              autoComplete="username"
              value={email}
              onChange={(e) => setEmail(e.target.value)}
            />
          </Field>
          <Field label="Password" error={errors.password}>
            <Input
              type="password"
              autoComplete="current-password"
              value={password}
              onChange={(e) => setPassword(e.target.value)}
            />
          </Field>
          {signIn.error && <Alert title="Could not sign in">{signIn.error}</Alert>}
          <Button type="submit" loading={signIn.busy} className="w-full">
            Sign in
          </Button>
        </form>
      </Card>
    </AuthLayout>
  );
}
