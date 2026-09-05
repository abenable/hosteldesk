import { useEffect, useState } from 'react';
import { Layout } from './components/Layout.tsx';
import type { View } from './components/types';
import { LockScreen } from './components/LockScreen.tsx';
import { SignIn } from './components/SignIn.tsx';
import { ToastContainer } from './components/ToastContainer.tsx';
import { Dashboard } from './views/Dashboard.tsx';
import { Residents } from './views/Residents.tsx';
import { Rooms } from './views/Rooms.tsx';
import { Invoices } from './views/Invoices.tsx';
import { Settings } from './views/Settings.tsx';
import { Onboarding } from './views/Onboarding.tsx';
import {
  getLicenseStatus,
  getOnboardingStatus,
  getSession,
  logout,
  type LicensePayload,
  type StaffUser,
} from './lib/tauri.ts';
import { setCurrentUser } from './lib/current-user.ts';
import { HostelProvider } from './lib/hostel-context.tsx';

export default function App() {
  const [view, setView] = useState<View>('dashboard');
  const [license, setLicense] = useState<LicensePayload | null>(null);
  const [onboardingComplete, setOnboardingComplete] = useState(false);
  const [currentUser, setCurrentUserState] = useState<StaffUser | null>(null);
  const [checking, setChecking] = useState(true);
  const [licenseError, setLicenseError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    Promise.all([getLicenseStatus(), getOnboardingStatus(), getSession()])
      .then(([license, onboardingComplete, user]) => {
        if (cancelled) return;
        setLicense(license);
        setOnboardingComplete(onboardingComplete);
        setCurrentUserState(user);
      })
      .catch((error) => {
        if (!cancelled) {
          setLicenseError(
            error instanceof Error ? error.message : String(error),
          );
        }
      })
      .finally(() => {
        if (!cancelled) setChecking(false);
      });
    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    const onComplete = (event: Event) => {
      const user = (event as CustomEvent<StaffUser | undefined>).detail;
      setOnboardingComplete(true);
      if (user) setCurrentUserState(user);
    };
    window.addEventListener('hosteldesk:onboarding-complete', onComplete);
    return () => {
      window.removeEventListener('hosteldesk:onboarding-complete', onComplete);
    };
  }, []);

  async function handleSignOut() {
    await logout();
    setCurrentUser(null);
    setCurrentUserState(null);
    setView('dashboard');
  }

  if (checking) {
    return (
      <div className="flex h-screen items-center justify-center overflow-y-auto bg-canvas text-ink">
        <p className="text-body-sm text-muted">Loading…</p>
      </div>
    );
  }

  if (licenseError) {
    return (
      <div className="flex h-screen items-center justify-center overflow-y-auto bg-canvas p-6 text-center">
        <div>
          <p className="text-display-sm text-ink">Could not start HostelDesk</p>
          <p className="mt-2 text-body-sm text-error-text">{licenseError}</p>
        </div>
      </div>
    );
  }

  if (!license) {
    return (
      <>
        <LockScreen onActivated={setLicense} />
        <ToastContainer />
      </>
    );
  }

  if (!onboardingComplete) {
    return (
      <>
        <Onboarding />
        <ToastContainer />
      </>
    );
  }

  if (!currentUser) {
    return (
      <>
        <SignIn onSignedIn={setCurrentUserState} />
        <ToastContainer />
      </>
    );
  }

  return (
    <HostelProvider>
      <AppShell
        view={view}
        onSelect={setView}
        currentUser={currentUser}
        onSignOut={handleSignOut}
      />
      <ToastContainer />
    </HostelProvider>
  );
}

function AppShell({
  view,
  onSelect,
  currentUser,
  onSignOut,
}: {
  view: View;
  onSelect: (view: View) => void;
  currentUser: StaffUser;
  onSignOut: () => Promise<void>;
}) {

  return (
    <Layout
      active={view}
      onSelect={onSelect}
      userName={currentUser.name || currentUser.username}
      userEmail={currentUser.username}
      onSignOut={onSignOut}
    >
      {view === 'dashboard' && <Dashboard onNavigate={onSelect} />}
      {view === 'residents' && <Residents onNavigate={onSelect} />}
      {view === 'rooms' && <Rooms />}
      {view === 'invoices' && <Invoices onNavigate={onSelect} />}
      {view === 'settings' && <Settings />}
    </Layout>
  );
}
