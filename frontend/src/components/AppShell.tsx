import { useState } from 'react';
import { useDesk, type Page, type Route } from '../lib/desk';
import { daysUntil, planLabel } from '../lib/format';
import DashboardPage from '../pages/dashboard/DashboardPage';
import PaymentsPage from '../pages/payments/PaymentsPage';
import ResidentDetailPage from '../pages/residents/ResidentDetailPage';
import ResidentsPage from '../pages/residents/ResidentsPage';
import RoomsPage from '../pages/rooms/RoomsPage';
import SettingsPage from '../pages/settings/SettingsPage';
import { Avatar, BrandMark, cx, DetailSlot, Dropdown, Icon, type IconName } from './ui';

interface NavItem {
  page: Page;
  label: string;
  icon: IconName;
}

const FRONT_DESK: NavItem[] = [
  { page: 'dashboard', label: 'Dashboard', icon: 'squares-2x2' },
  { page: 'rooms', label: 'Rooms', icon: 'home' },
  { page: 'residents', label: 'Residents', icon: 'user-group' },
  { page: 'payments', label: 'Payments', icon: 'banknotes' },
];
const SETTINGS: NavItem = { page: 'settings', label: 'Settings', icon: 'cog-6-tooth' };

function CurrentPage({ route }: { route: Route }) {
  switch (route.page) {
    case 'dashboard':
      return <DashboardPage />;
    case 'rooms':
      return <RoomsPage />;
    case 'residents':
      return route.residentId === undefined ? (
        <ResidentsPage />
      ) : (
        <ResidentDetailPage residentId={route.residentId} />
      );
    case 'payments':
      return <PaymentsPage />;
    case 'settings':
      return <SettingsPage />;
  }
}

/** Sidebar link: outline icon, label, optional right-aligned count; the active one is a filled pill. */
function NavLink({ item, count }: { item: NavItem; count?: number }) {
  const { route, navigate } = useDesk();
  const active = item.page === route.page;
  return (
    <li>
      <button
        type="button"
        aria-current={active ? 'page' : undefined}
        onClick={() => navigate({ page: item.page })}
        className={cx(
          'flex w-full items-center gap-3 rounded-lg px-3 py-2 text-sm font-medium transition-colors focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-primary-900',
          active
            ? 'bg-primary-100 text-gray-900'
            : 'text-gray-600 hover:bg-gray-50 hover:text-gray-900',
        )}
      >
        <Icon name={item.icon} className={cx('size-5', active ? 'text-gray-900' : 'text-gray-500')} />
        <span className="flex-1 text-start">{item.label}</span>
        {count !== undefined && (
          <span className="text-xs font-normal text-gray-500 tabular-nums">{count}</span>
        )}
      </button>
    </li>
  );
}

/** Hostel name with the plan and days left under it (amber within 30 days, red within 7). */
function Workspace() {
  const { profile, license, formatDate } = useDesk();
  const days = license ? daysUntil(license.expires_on) : null;
  return (
    <div className="flex items-center gap-3 border-b border-gray-200 p-4">
      <BrandMark />
      <div className="min-w-0">
        <p className="truncate text-sm font-semibold text-gray-900" title={profile.name}>
          {profile.name}
        </p>
        {license && days !== null ? (
          <p
            title={`Until ${formatDate(license.expires_on)}`}
            className={cx(
              'truncate text-xs',
              days <= 7 ? 'text-red-600' : days <= 30 ? 'text-amber-700' : 'text-gray-500',
            )}
          >
            {planLabel(license.plan)} · {days === 1 ? '1 day left' : `${Math.max(days, 0)} days left`}
          </p>
        ) : (
          <p className="text-xs text-gray-500">HostelDesk</p>
        )}
      </div>
    </div>
  );
}

/** Background sync status; click to sync now. Hidden on a desk that does not sync. */
function SyncStatus() {
  const { sync, syncNow } = useDesk();
  if (!sync.enrolled) return null;
  const time = sync.lastAt?.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
  const [dot, text] = sync.busy
    ? ['bg-gray-400 animate-pulse', 'Syncing…']
    : sync.error
      ? ['bg-amber-500', 'Sync failed, retrying']
      : time
        ? ['bg-green-500', `Synced ${time}`]
        : ['bg-gray-300', 'Waiting to sync'];
  return (
    <button
      type="button"
      onClick={syncNow}
      disabled={sync.busy}
      title={sync.error ?? 'Sync now'}
      className="flex w-full items-center gap-2 rounded-lg px-3 py-1.5 text-xs text-gray-500 transition-colors hover:bg-gray-50 hover:text-gray-900 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-primary-900 disabled:cursor-wait"
    >
      <span aria-hidden="true" className={cx('size-2 shrink-0 rounded-full', dot)} />
      <span aria-live="polite" className="truncate">
        {text}
      </span>
    </button>
  );
}

/** The signed-in user, pinned to the bottom of the sidebar; Sign out is in its menu. */
function UserCard() {
  const { user, appVersion, signOut } = useDesk();
  const role = user.role === 'admin' ? 'Administrator' : 'Manager';
  return (
    <div className="flex items-center gap-3 px-1">
      <Avatar name={user.full_name} />
      <div className="min-w-0 flex-1">
        <p className="truncate text-sm font-medium text-gray-900">{user.full_name}</p>
        <p className="truncate text-xs text-gray-500">{role}</p>
      </div>
      <Dropdown
        label={`${user.full_name}, account menu`}
        trigger={<Icon name="ellipsis-vertical" />}
        header={
          <p className="text-xs text-gray-500">
            <span className="block truncate text-sm font-medium text-gray-900">{user.email}</span>
            HostelDesk {appVersion}
          </p>
        }
        items={[{ label: 'Sign out', onSelect: signOut }]}
      />
    </div>
  );
}

/**
 * The signed-in layout (HyperUI app-shells "Sidebar with page header" +
 * vertical-menu "Base with icon", tuned to the Fernhill/Tabula references):
 * two white panels on the grey canvas. The sidebar holds the hostel, the
 * navigation, sync status and the user card; the content panel holds the
 * current page (its PageHeader is the title area) and, on the right, the
 * column a `<DetailPanel>` docks into. Desktop only (≥ 1024 px).
 */
export default function AppShell() {
  const { route, profile } = useDesk();
  const [slot, setSlot] = useState<HTMLElement | null>(null);

  return (
    <div className="flex h-screen gap-3 p-3">
      <aside className="flex w-60 shrink-0 flex-col overflow-hidden rounded-2xl border border-gray-200 bg-white shadow-xs xl:w-64">
        <Workspace />
        <nav aria-label="Main" className="flex min-h-0 flex-1 flex-col overflow-y-auto p-3">
          <p className="px-3 pt-1 pb-2 text-xs font-medium text-gray-500">Front desk</p>
          <ul className="space-y-1">
            {FRONT_DESK.map((item) => (
              <NavLink
                key={item.page}
                item={item}
                count={item.page === 'rooms' ? profile.room_count : undefined}
              />
            ))}
          </ul>
          <ul className="mt-auto pt-3">
            <NavLink item={SETTINGS} />
          </ul>
        </nav>
        <div className="space-y-2 border-t border-gray-200 p-3">
          <SyncStatus />
          <UserCard />
        </div>
      </aside>

      <div className="flex min-w-0 flex-1 overflow-hidden rounded-2xl border border-gray-200 bg-white shadow-xs">
        <DetailSlot value={slot}>
          {/* Keyed by location: a new page starts at the top with fresh state; a tab change keeps both. */}
          <main
            key={`${route.page}:${route.residentId ?? ''}`}
            className="min-w-0 flex-1 overflow-y-auto"
          >
            <div className="mx-auto max-w-7xl space-y-6 px-8 py-8">
              <CurrentPage route={route} />
            </div>
          </main>
        </DetailSlot>
        <div ref={setSlot} className="w-80 shrink-0 border-s border-gray-200 empty:hidden xl:w-96" />
      </div>
    </div>
  );
}
