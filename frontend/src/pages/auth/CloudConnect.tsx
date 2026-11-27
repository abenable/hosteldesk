import { useState, type FormEvent } from 'react';
import { Alert, Button, Field, Icon, Input, Tabs } from '../../components/ui';
import * as api from '../../lib/api';
import { looksLikeEmail } from '../../lib/format';
import { toast } from '../../lib/toast';
import type { ConnectResult } from '../../lib/types';
import { useAction } from '../../lib/useLoad';

type Mode = 'sign_in' | 'create';
type Errors = Partial<Record<'organization' | 'email' | 'password', string>>;

/**
 * Connect this computer to a HostelDesk account: "Sign in" or "Create account"
 * (two modes), with the server URL under "Advanced". Runs `connect_cloud`
 * (which also syncs once), toasts the outcome, then calls `onConnected`.
 *
 * Used by the Lock screen (`onConnected` re-boots) and by Settings → Cloud for
 * an admin on a desk that is not connected yet (`onConnected={() => desk.reload()}`).
 */
export default function CloudConnectForm({
  onConnected,
}: {
  onConnected: (result: ConnectResult) => void | Promise<void>;
}) {
  const [mode, setMode] = useState<Mode>('sign_in');
  const [organization, setOrganization] = useState('');
  const [email, setEmail] = useState('');
  const [password, setPassword] = useState('');
  const [server, setServer] = useState('');
  const [errors, setErrors] = useState<Errors>({});
  const connect = useAction();
  const create = mode === 'create';

  const submit = (event: FormEvent) => {
    event.preventDefault();
    const found: Errors = {};
    if (create && !organization.trim()) found.organization = 'Enter the name of your hostel or business.';
    if (!looksLikeEmail(email)) found.email = 'Enter a valid email address.';
    if (!password) found.password = 'Enter your password.';
    else if (create && [...password].length < 8) found.password = 'Use at least 8 characters.';
    setErrors(found);
    if (Object.keys(found).length > 0) return;

    void connect.run(async () => {
      const result = await api.connectCloud({
        server_url: server.trim(),
        email: email.trim(),
        password,
        create_account: create,
        organization_name: create ? organization.trim() : '',
      });
      setPassword('');
      if (result.sync_error) {
        toast({ tone: 'warning', title: 'Connected. Sync will retry.', message: result.sync_error });
      } else if (!result.licensed) {
        toast({
          tone: 'warning',
          title: create ? 'Account created' : 'Signed in',
          message: result.lock_reason ?? 'This computer is still locked.',
        });
      } else {
        toast({ tone: 'success', title: create ? 'Account created.' : 'Signed in.' });
      }
      await onConnected(result);
    });
  };

  return (
    <form onSubmit={submit} noValidate className="space-y-5">
      <Tabs
        full
        label="Account"
        value={mode}
        onChange={(next) => {
          setMode(next);
          setErrors({});
          connect.setError(null);
        }}
        items={[
          { value: 'sign_in', label: 'Sign in' },
          { value: 'create', label: 'Create account' },
        ]}
      />
      <p className="text-sm text-gray-600">
        {create
          ? 'New to HostelDesk? Create your account; your free trial starts once it is approved.'
          : 'Use the email and password of your HostelDesk account.'}
      </p>
      {create && (
        <Field label="Hostel or business name" error={errors.organization}>
          <Input
            autoComplete="organization"
            value={organization}
            onChange={(e) => setOrganization(e.target.value)}
          />
        </Field>
      )}
      <Field label="Email" error={errors.email}>
        <Input type="email" autoComplete="email" value={email} onChange={(e) => setEmail(e.target.value)} />
      </Field>
      <Field
        label="Password"
        hint={create ? 'At least 8 characters.' : undefined}
        error={errors.password}
      >
        <Input
          type="password"
          autoComplete={create ? 'new-password' : 'current-password'}
          value={password}
          onChange={(e) => setPassword(e.target.value)}
        />
      </Field>
      <details className="group">
        <summary className="flex w-fit cursor-pointer list-none items-center gap-1 rounded-sm text-sm font-medium text-gray-600 hover:text-gray-900 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-primary-900 [&::-webkit-details-marker]:hidden">
          Advanced
          <Icon name="chevron-down" className="size-4 transition group-open:-rotate-180" />
        </summary>
        <Field label="Server" hint="Leave blank to use HostelDesk Cloud." className="mt-3">
          <Input
            type="url"
            inputMode="url"
            placeholder="https://"
            value={server}
            onChange={(e) => setServer(e.target.value)}
          />
        </Field>
      </details>
      {connect.error && (
        <Alert title={create ? 'Could not create the account' : 'Could not sign in'}>
          {connect.error}
        </Alert>
      )}
      <Button type="submit" loading={connect.busy} className="w-full">
        {create ? 'Create account' : 'Sign in'}
      </Button>
    </form>
  );
}
