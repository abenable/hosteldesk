import { useEffect, useRef } from 'react';
import { Alert, Button, Card } from '../../components/ui';
import * as api from '../../lib/api';
import { toast } from '../../lib/toast';
import type { BootState } from '../../lib/types';
import { useAction } from '../../lib/useLoad';
import AuthLayout from './AuthLayout';
import CloudConnectForm from './CloudConnect';

const OFFLINE =
  'This computer needs to check its license with HostelDesk. Connect it to the internet; it checks again by itself.';

/**
 * HostelDesk is licensed per account, so a computer unlocks by signing in.
 * A computer that already signed in (enrolled) only needs the server to
 * confirm the license again: it shows why it is locked, a "Check again"
 * button, and unlocks by itself when the background sync reports a license.
 */
export default function Lock({ boot, reboot }: { boot: BootState; reboot: () => Promise<void> }) {
  const check = useAction();

  useEffect(
    () =>
      api.onSync((event) => {
        if (event.licensed) void reboot();
      }),
    [reboot],
  );

  // The background loop's first pass waits 30 s (its event would beat our listener).
  const started = useRef(false);
  useEffect(() => {
    if (started.current || !boot.enrolled) return;
    started.current = true;
    api.syncNow().then(
      (result) => {
        if (result.licensed) void reboot();
      },
      () => undefined, // "Check again" shows errors; the background loop retries.
    );
  }, [boot.enrolled, reboot]);

  const checkAgain = () =>
    void check.run(async () => {
      const result = await api.syncNow();
      await reboot();
      if (result.licensed) return;
      if (result.error) throw new Error(`Could not reach HostelDesk: ${result.error}`);
      toast({ tone: 'warning', title: 'Still locked', message: 'HostelDesk answered, but this account is not active yet.' });
    });

  if (!boot.enrolled) {
    return (
      <AuthLayout
        title="Sign in to HostelDesk"
        description="Sign in to your HostelDesk account to use this computer. New to HostelDesk? Create an account to start your free trial."
        footer={`HostelDesk ${boot.app_version}`}
      >
        <Card>
          <CloudConnectForm onConnected={reboot} />
        </Card>
      </AuthLayout>
    );
  }

  return (
    <AuthLayout title="This computer is locked" footer={`HostelDesk ${boot.app_version}`}>
      <Alert
        tone="warning"
        title="Waiting for your license"
        action={
          <Button size="sm" variant="secondary" icon="arrow-path" loading={check.busy} onClick={checkAgain}>
            Check again
          </Button>
        }
      >
        {boot.lock_reason ?? OFFLINE}
      </Alert>
      {check.error && <Alert title="Check failed">{check.error}</Alert>}
      <div className="flex items-center gap-4 text-sm text-gray-600">
        <span className="h-px flex-1 bg-gray-200" />
        Or sign in with a different account
        <span className="h-px flex-1 bg-gray-200" />
      </div>
      <Card>
        <CloudConnectForm onConnected={reboot} />
      </Card>
    </AuthLayout>
  );
}
