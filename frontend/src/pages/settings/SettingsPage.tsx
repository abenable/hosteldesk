import { PageHeader, Tabs, type TabItem } from '../../components/ui';
import { useDesk } from '../../lib/desk';
import AccountSection from './AccountSection';
import BackupSection from './BackupSection';
import BusinessSection from './BusinessSection';
import CloudSection from './CloudSection';
import PreferencesSection from './PreferencesSection';
import TeamSection from './TeamSection';

type Tab = 'account' | 'business' | 'team' | 'preferences' | 'backup' | 'cloud';

const TABS: TabItem<Tab>[] = [
  { value: 'account', label: 'Account' },
  { value: 'business', label: 'Business' },
  { value: 'team', label: 'Team' },
  { value: 'preferences', label: 'Preferences' },
  { value: 'backup', label: 'Backup' },
  { value: 'cloud', label: 'Cloud' },
];

/** What a manager (non-admin) may open, as in main's ui.rs. */
const STAFF_TABS: Tab[] = ['account', 'cloud'];

/**
 * Account, business, team, preferences, backups and cloud, as segmented tabs
 * (`route.tab` picks one; unknown or admin-only tabs fall back to Account).
 * Each tab is a stack of two-column sections (see `Section`).
 */
export default function SettingsPage() {
  const desk = useDesk();
  const items = desk.isAdmin ? TABS : TABS.filter((t) => STAFF_TABS.includes(t.value));
  const tab = items.find((t) => t.value === desk.route.tab)?.value ?? 'account';
  const role = desk.isAdmin ? 'Administrator' : 'Manager';

  return (
    <>
      <PageHeader
        title="Settings"
        description={`${desk.profile.name} · ${desk.user.full_name}, ${role} · HostelDesk ${desk.appVersion}`}
      />
      <Tabs
        label="Settings sections"
        value={tab}
        onChange={(next) => desk.navigate({ page: 'settings', tab: next })}
        items={items}
      />
      <div className="@container divide-y divide-gray-200 pt-2">
        {tab === 'account' && <AccountSection />}
        {tab === 'business' && <BusinessSection />}
        {tab === 'team' && <TeamSection />}
        {tab === 'preferences' && <PreferencesSection />}
        {tab === 'backup' && <BackupSection />}
        {tab === 'cloud' && <CloudSection />}
      </div>
    </>
  );
}
