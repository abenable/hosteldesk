import { useCallback, useEffect, useRef, useState } from 'react';
import AppShell from './components/AppShell';
import DeskProvider from './components/DeskProvider';
import { Spinner, Toaster } from './components/ui';
import * as api from './lib/api';
import { clearToasts, toast } from './lib/toast';
import type { BootState } from './lib/types';
import BootFailed from './pages/auth/BootFailed';
import Lock from './pages/auth/Lock';
import Onboarding from './pages/auth/Onboarding';
import SignIn from './pages/auth/SignIn';

type Gate = { kind: 'boot' } | { kind: 'failed'; message: string } | { kind: 'gate'; boot: BootState };

/**
 * The gate. `boot_state` decides the screen: Lock, Onboarding, SignIn, or the
 * AppShell for the signed-in user. Every transition (sign-in, onboarding,
 * connect, sign-out, session timeout, lock) goes back through `boot_state`, so
 * Rust stays the only judge of which screen is allowed.
 */
export default function App() {
  const [gate, setGate] = useState<Gate>({ kind: 'boot' });
  const latest = useRef(0);

  /** Re-run boot_state; the current screen stays up until the answer arrives. */
  const reboot = useCallback(() => {
    const id = ++latest.current;
    return api.bootState().then(
      (state) => {
        if (id === latest.current) setGate({ kind: 'gate', boot: state });
      },
      (error: unknown) => {
        if (id === latest.current) setGate({ kind: 'failed', message: api.errorMessage(error) });
      },
    );
  }, []);
  /** Boot from scratch behind the full-screen spinner ("Try again"). */
  const boot = useCallback(() => {
    setGate({ kind: 'boot' });
    void reboot();
  }, [reboot]);

  // The initial state already shows the spinner.
  useEffect(() => {
    void reboot();
  }, [reboot]);

  // Guard errors from any command: a lapsed session or license goes back to the gate.
  useEffect(
    () =>
      api.onGuardError((code) => {
        if (code === 'forbidden') {
          toast({ tone: 'error', title: 'Only an administrator can do that.' });
          return;
        }
        clearToasts();
        if (code === 'unauthorized') toast({ tone: 'warning', title: 'Your session timed out. Sign in again.' });
        void reboot();
      }),
    [reboot],
  );

  return (
    <>
      <GateScreen gate={gate} boot={boot} reboot={reboot} />
      <Toaster />
    </>
  );
}

function GateScreen({
  gate,
  boot,
  reboot,
}: {
  gate: Gate;
  boot: () => void;
  reboot: () => Promise<void>;
}) {
  if (gate.kind === 'boot') {
    return (
      <div className="grid min-h-screen place-items-center">
        <Spinner size="lg" label="Starting HostelDesk…" />
      </div>
    );
  }
  if (gate.kind === 'failed') return <BootFailed message={gate.message} onRetry={boot} />;

  const state = gate.boot;
  switch (state.gate) {
    case 'lock':
      return <Lock boot={state} reboot={reboot} />;
    case 'onboarding':
      return <Onboarding boot={state} reboot={reboot} />;
    case 'sign_in':
      return <SignIn boot={state} reboot={reboot} />;
    case 'ready':
      if (!state.user) return <BootFailed message="No staff member is signed in." onRetry={boot} />;
      return (
        // Keyed by user: a new session never sees the last one's state.
        <DeskProvider key={state.user.id} boot={state} user={state.user} reboot={reboot}>
          <AppShell />
        </DeskProvider>
      );
  }
}
