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

const FOCUS = 'focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-primary-900';

// A per-computer convenience: storage can be blocked, so it falls back to expanded.
const COLLAPSED_KEY = 'hosteldesk.sidebar.collapsed';
function readCollapsed(): boolean {
  try {
    return localStorage.getItem(COLLAPSED_KEY) === '1';
  } catch {
    return false;
  }
}
function saveCollapsed(collapsed: boolean) {
  try {
    localStorage.setItem(COLLAPSED_KEY, collapsed ? '1' : '0');
  } catch {
    // ignore: the sidebar just opens expanded next time
  }
}

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

/**
 * Sidebar link: outline icon, label, optional right-aligned count; the active one is a
 * filled pill. Collapsed, only the icon shows (the label stays for screen readers and as a tooltip).
 */
function NavLink({ item, count, collapsed }: { item: NavItem; count?: number; collapsed: boolean }) {
  const { route, navigate } = useDesk();
  const active = item.page === route.page;
  return (
    <li>
      <button
        type="button"
        aria-current={active ? 'page' : undefined}
        title={collapsed ? item.label : undefined}
        onClick={() => navigate({ page: item.page })}
        className={cx(
          'flex w-full items-center gap-3 rounded-full py-2 text-sm font-medium transition-colors',
          FOCUS,
          collapsed ? 'justify-center px-2' : 'px-3',
          active
            ? 'bg-primary-100 text-gray-900'
            : 'text-gray-600 hover:bg-gray-50 hover:text-gray-900',
        )}
      >
        <Icon name={item.icon} className={cx('size-5 shrink-0', active ? 'text-gray-900' : 'text-gray-500')} />
        <span className={collapsed ? 'sr-only' : 'flex-1 text-start'}>{item.label}</span>
        {count !== undefined && !collapsed && (
          <span className="text-xs font-normal text-gray-500 tabular-nums">{count}</span>
        )}
      </button>
    </li>
  );
}

/** Collapses the sidebar to an icon rail, or expands it again. */
function CollapseButton({ collapsed, onToggle }: { collapsed: boolean; onToggle: () => void }) {
  const label = collapsed ? 'Expand sidebar' : 'Collapse sidebar';
  return (
    <button
      type="button"
      aria-controls="sidebar"
      aria-expanded={!collapsed}
      aria-label={label}
      title={label}
      onClick={onToggle}
      className={cx(
        'grid size-8 shrink-0 place-items-center rounded-full text-gray-500 transition-colors hover:bg-gray-100 hover:text-gray-900',
        FOCUS,
      )}
    >
      <Icon name={collapsed ? 'chevron-right' : 'chevron-left'} className="size-5" />
    </button>
  );
}

/**
 * Hostel name with the plan and days left under it (amber within 30 days, red within 7);
 * the plan line opens Settings → Subscription. The collapse button sits at the
 * right; collapsed, the logo shows with the expand button under it.
 */
function Workspace({ collapsed, onToggle }: { collapsed: boolean; onToggle: () => void }) {
  const { profile, license, formatDate, navigate } = useDesk();
  const days = license ? daysUntil(license.expires_on) : null;
  if (collapsed) {
    return (
      <div className="flex flex-col items-center gap-2 border-b border-gray-200 py-4">
        <span title={profile.name}>
          <BrandMark />
        </span>
        <CollapseButton collapsed onToggle={onToggle} />
      </div>
    );
  }
  return (
    <div className="flex items-center gap-3 border-b border-gray-200 p-4">
      <BrandMark />
      <div className="min-w-0 flex-1">
        <p className="truncate text-sm font-semibold text-gray-900" title={profile.name}>
          {profile.name}
        </p>
        {license && days !== null ? (
          <button
            type="button"
            onClick={() => navigate({ page: 'settings', tab: 'subscription' })}
            title={`Until ${formatDate(license.expires_on)}. Manage subscription`}
            className={cx(
              'block max-w-full truncate rounded-full text-xs hover:underline',
              FOCUS,
              days <= 7 ? 'text-red-600' : days <= 30 ? 'text-amber-700' : 'text-gray-500',
            )}
          >
            {planLabel(license.plan)} · {days === 1 ? '1 day left' : `${Math.max(days, 0)} days left`}
          </button>
        ) : (
          <p className="text-xs text-gray-500">HostelDesk</p>
        )}
      </div>
      <CollapseButton collapsed={false} onToggle={onToggle} />
    </div>
  );
}

/** Background sync status; click to sync now. Hidden on a desk that does not sync. Collapsed, only the dot shows. */
function SyncStatus({ collapsed }: { collapsed: boolean }) {
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
      title={sync.error ?? (collapsed ? `${text}. Sync now` : 'Sync now')}
      className={cx(
        'flex w-full items-center gap-2 rounded-full py-1.5 text-xs text-gray-500 transition-colors hover:bg-gray-50 hover:text-gray-900 disabled:cursor-wait',
        FOCUS,
        collapsed ? 'justify-center px-2' : 'px-3',
      )}
    >
      <span aria-hidden="true" className={cx('size-2 shrink-0 rounded-full', dot)} />
      <span aria-live="polite" className={collapsed ? 'sr-only' : 'truncate'}>
        {text}
      </span>
    </button>
  );
}

/** The signed-in user, pinned to the bottom of the sidebar; Sign out is in its menu. Collapsed, the avatar opens the menu. */
function UserCard({ collapsed }: { collapsed: boolean }) {
  const { user, appVersion, signOut } = useDesk();
  const role = user.role === 'admin' ? 'Administrator' : 'Manager';
  const header = (
    <p className="text-xs text-gray-500">
      <span className="block truncate text-sm font-medium text-gray-900">{user.email}</span>
      HostelDesk {appVersion}
    </p>
  );
  const items = [{ label: 'Sign out', onSelect: signOut }];
  if (collapsed) {
    return (
      <div className="flex justify-center">
        <Dropdown
          label={`${user.full_name}, account menu`}
          trigger={<Avatar name={user.full_name} />}
          triggerClassName={cx('rounded-full', FOCUS)}
          header={header}
          items={items}
        />
      </div>
    );
  }
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
        header={header}
        items={items}
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
 * column a `<DetailPanel>` docks into. The content uses the full width of its
 * panel; the sidebar collapses to an icon rail (remembered per computer).
 * Desktop only (≥ 1024 px).
 */
export default function AppShell() {
  const { route, profile } = useDesk();
  const [slot, setSlot] = useState<HTMLElement | null>(null);
  const [collapsed, setCollapsed] = useState(readCollapsed);
  const toggle = () => {
    saveCollapsed(!collapsed);
    setCollapsed(!collapsed);
  };

  return (
    <div className="flex h-screen gap-3 p-3">
      <aside
        id="sidebar"
        className={cx(
          'flex shrink-0 flex-col overflow-hidden rounded-2xl border border-gray-200 bg-white shadow-xs',
          collapsed ? 'w-18' : 'w-60 xl:w-64',
        )}
      >
        <Workspace collapsed={collapsed} onToggle={toggle} />
        <nav aria-label="Main" className="flex min-h-0 flex-1 flex-col overflow-y-auto p-3">
          <p className={cx('px-3 pt-1 pb-2 text-xs font-medium text-gray-500', collapsed && 'sr-only')}>
            Front desk
          </p>
          <ul className="space-y-1">
            {FRONT_DESK.map((item) => (
              <NavLink
                key={item.page}
                item={item}
                collapsed={collapsed}
                count={item.page === 'rooms' ? profile.room_count : undefined}
              />
            ))}
          </ul>
          <ul className="mt-auto space-y-1 pt-3">
            <NavLink item={SETTINGS} collapsed={collapsed} />
          </ul>
        </nav>
        <div className={cx('space-y-2 border-t border-gray-200', collapsed ? 'px-2 py-3' : 'p-3')}>
          <SyncStatus collapsed={collapsed} />
          <UserCard collapsed={collapsed} />
        </div>
      </aside>

      <div className="flex min-w-0 flex-1 overflow-hidden rounded-2xl border border-gray-200 bg-white shadow-xs">
        <DetailSlot value={slot}>
          {/* Keyed by location: a new page starts at the top with fresh state; a tab change keeps both. */}
          <main
            key={`${route.page}:${route.residentId ?? ''}`}
            className="min-w-0 flex-1 overflow-y-auto"
          >
            <div className="space-y-6 px-8 py-8">
              <CurrentPage route={route} />
            </div>
          </main>
        </DetailSlot>
        <div ref={setSlot} className="w-80 shrink-0 border-s border-gray-200 empty:hidden xl:w-96" />
      </div>
    </div>
  );
}
