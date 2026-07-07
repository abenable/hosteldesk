import { useEffect, useState } from 'react';
import { Layout } from './components/Layout.tsx';
import type { View } from './components/Sidebar.tsx';
import { LockScreen } from './components/LockScreen.tsx';
import { ToastContainer } from './components/ToastContainer.tsx';
import { Dashboard } from './views/Dashboard.tsx';
import { Residents } from './views/Residents.tsx';
import { Rooms } from './views/Rooms.tsx';
import { Invoices } from './views/Invoices.tsx';
import { Settings } from './views/Settings.tsx';
import { Onboarding } from './views/Onboarding.tsx';
import { getLicenseStatus, type LicensePayload } from './lib/tauri.ts';
import { isOnboardingComplete, getHostelProfile } from './lib/onboarding.ts';

const views: Record<View, React.ComponentType> = {
  dashboard: Dashboard,
  rooms: Rooms,
  residents: Residents,
  invoices: Invoices,
  settings: Settings,
};

export default function App() {
  const [view, setView] = useState<View>('dashboard');
  const [license, setLicense] = useState<LicensePayload | null>(null);
  const [checking, setChecking] = useState(true);
  const [licenseError, setLicenseError] = useState<string | null>(null);
  const [onboardingDone, setOnboardingDone] = useState(isOnboardingComplete());

  useEffect(() => {
    getLicenseStatus()
      .then(setLicense)
      .catch((e) =>
        setLicenseError(e instanceof Error ? e.message : String(e)),
      )
      .finally(() => setChecking(false));
  }, []);

  useEffect(() => {
    const onComplete = () => setOnboardingDone(true);
    const onReset = () => setOnboardingDone(false);
    window.addEventListener('hosteldesk:onboarding-complete', onComplete);
    window.addEventListener('hosteldesk:onboarding-reset', onReset);
    return () => {
      window.removeEventListener('hosteldesk:onboarding-complete', onComplete);
      window.removeEventListener('hosteldesk:onboarding-reset', onReset);
    };
  }, []);

  if (checking) {
    return (
      <div className="flex min-h-screen items-center justify-center bg-canvas text-ink">
        <p className="text-body-sm text-muted">Loading…</p>
      </div>
    );
  }

  if (licenseError) {
    return (
      <div className="flex min-h-screen items-center justify-center bg-canvas p-6 text-center">
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

  if (!onboardingDone) {
    return (
      <>
        <Onboarding />
        <ToastContainer />
      </>
    );
  }

  const ViewComponent = views[view];
  const hostel = getHostelProfile();
  const userName = 'A'; // ponytail: real session user pending login wiring

  return (
    <>
      <Layout
        active={view}
        onSelect={setView}
        hostelName={hostel?.name}
        userName={userName}
      >
        {view === 'residents' ? (
          <Residents onNavigate={setView} />
        ) : (
          <ViewComponent />
        )}
      </Layout>
      <ToastContainer />
    </>
  );
}
