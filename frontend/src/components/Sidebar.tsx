import logo from '../assets/logo.png';
import {
  DashboardIcon,
  RoomsIcon,
  ResidentsIcon,
  InvoiceIcon,
  SettingsIcon,
  ChevronLeftIcon,
  ChevronRightIcon,
} from './ui/icons.tsx';

export type View = 'dashboard' | 'rooms' | 'residents' | 'invoices' | 'settings';

const items: { id: View; label: string; icon: typeof DashboardIcon }[] = [
  { id: 'dashboard', label: 'Dashboard', icon: DashboardIcon },
  { id: 'rooms', label: 'Rooms', icon: RoomsIcon },
  { id: 'residents', label: 'Residents', icon: ResidentsIcon },
  { id: 'invoices', label: 'Invoices', icon: InvoiceIcon },
  { id: 'settings', label: 'Settings', icon: SettingsIcon },
];

export function Sidebar({
  active,
  onSelect,
  collapsed = false,
  onToggleCollapse,
}: {
  active: View;
  onSelect: (view: View) => void;
  collapsed?: boolean;
  onToggleCollapse?: () => void;
}) {
  return (
    <nav className="flex h-full flex-col gap-1 px-3 py-6">
      <div
        className={`flex items-center gap-3 px-2 pb-6 ${
          collapsed ? 'justify-center' : ''
        }`}
      >
        <img src={logo} alt="HostelDesk" className="h-10 w-10 shrink-0 rounded-lg" />
        {!collapsed && (
          <span className="text-display-md text-primary">HostelDesk</span>
        )}
      </div>

      {items.map((item) => {
        const selected = item.id === active;
        const Icon = item.icon;
        return (
          <button
            key={item.id}
            type="button"
            onClick={() => onSelect(item.id)}
            title={collapsed ? item.label : undefined}
            className={`flex items-center rounded-full text-nav-link transition-colors ${
              collapsed ? 'justify-center p-3' : 'gap-3 px-3 py-2.5'
            } ${
              selected
                ? 'bg-surface-strong text-ink'
                : 'text-muted hover:bg-surface-soft hover:text-ink'
            }`}
            aria-current={selected ? 'page' : undefined}
            aria-label={collapsed ? item.label : undefined}
          >
            <Icon width={20} height={20} />
            {!collapsed && item.label}
          </button>
        );
      })}

      <div className="mt-auto">
        {onToggleCollapse && (
          <button
            type="button"
            onClick={onToggleCollapse}
            title={collapsed ? 'Expand' : 'Collapse'}
            className={`flex items-center rounded-full text-muted transition-colors hover:bg-surface-soft hover:text-ink ${
              collapsed ? 'justify-center p-3' : 'gap-3 px-3 py-2.5'
            }`}
            aria-label={collapsed ? 'Expand sidebar' : 'Collapse sidebar'}
          >
            {collapsed ? (
              <ChevronRightIcon width={20} height={20} />
            ) : (
              <>
                <ChevronLeftIcon width={20} height={20} />
                Collapse
              </>
            )}
          </button>
        )}
      </div>
    </nav>
  );
}
