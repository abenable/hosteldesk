import { useState } from 'react';
import { useHostel } from '../hooks/use-hostel.ts';
import { getCurrentUser } from '../lib/current-user.ts';
import { BusinessSection } from './settings/BusinessSection.tsx';
import { TeamSection } from './settings/TeamSection.tsx';
import { DataSecuritySection } from './settings/DataSecuritySection.tsx';
import { PreferencesSection } from './settings/PreferencesSection.tsx';
import { AccountSection } from './settings/AccountSection.tsx';

const SECTIONS = [
  { id: 'account', label: 'Account' },
  { id: 'business', label: 'Hostel' },
  { id: 'team', label: 'Team' },
  { id: 'data', label: 'Data & Security' },
  { id: 'preferences', label: 'Preferences' },
] as const;

type SectionId = (typeof SECTIONS)[number]['id'];

export function Settings() {
  const { hostelId, refresh: refreshHostel } = useHostel();
  const [section, setSection] = useState<SectionId>('business');
  const currentUser = getCurrentUser();

  return (
    <div className="flex flex-col gap-6 md:flex-row md:gap-8">
      <nav
        className="flex gap-1 overflow-x-auto md:w-56 md:flex-col"
        aria-label="Settings sections"
      >
        {SECTIONS.map((s) => (
          <button
            key={s.id}
            onClick={() => setSection(s.id)}
            className={`whitespace-nowrap rounded-full px-4 py-2.5 text-left text-body-md transition-colors duration-200 ease-out ${
              section === s.id
                ? 'bg-primary/10 font-semibold text-primary'
                : 'text-muted hover:text-ink'
            }`}
          >
            {s.label}
          </button>
        ))}
      </nav>

      <div className="mx-auto min-w-0 w-full max-w-3xl flex-1">
        {section === 'business' && (
          <BusinessSection hostelId={hostelId} onSaved={refreshHostel} />
        )}
        {section === 'team' && <TeamSection hostelId={hostelId} />}
        {section === 'data' && <DataSecuritySection />}
        {section === 'preferences' && (
          <PreferencesSection hostelId={hostelId} />
        )}
        {section === 'account' && currentUser && (
          <AccountSection user={currentUser} />
        )}
      </div>
    </div>
  );
}
