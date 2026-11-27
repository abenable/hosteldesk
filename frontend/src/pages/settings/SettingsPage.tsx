import { PageHeader, Tabs, type TabItem } from '../../components/ui';
import { useDesk } from '../../lib/desk';
import AccountSection from './AccountSection';
import BusinessSection from './BusinessSection';
import PreferencesSection from './PreferencesSection';
import SubscriptionSection from './SubscriptionSection';
import TeamSection from './TeamSection';

type Tab = 'account' | 'subscription' | 'business' | 'team' | 'preferences';

const TABS: TabItem<Tab>[] = [
  { value: 'account', label: 'Account' },
  { value: 'subscription', label: 'Subscription' },
  { value: 'business', label: 'Business' },
  { value: 'team', label: 'Team' },
  { value: 'preferences', label: 'Preferences' },
];

/** What a manager (non-admin) may open, as in main's ui.rs. */
const STAFF_TABS: Tab[] = ['account', 'subscription'];

/**
 * Account, subscription, business, team and preferences, as segmented tabs
 * (`route.tab` picks one; unknown or admin-only tabs fall back to Account).
 * Each tab is a single column of sections (see `Section`).
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
      <div className="max-w-2xl divide-y divide-gray-200 pt-2">
        {tab === 'account' && <AccountSection />}
        {tab === 'subscription' && <SubscriptionSection />}
        {tab === 'business' && <BusinessSection />}
        {tab === 'team' && <TeamSection />}
        {tab === 'preferences' && <PreferencesSection />}
      </div>
    </>
  );
}
