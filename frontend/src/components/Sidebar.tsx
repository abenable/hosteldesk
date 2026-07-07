import logo from '../assets/logo.png';
import {
  DashboardIcon,
  RoomsIcon,
  ResidentsIcon,
  InvoiceIcon,
  SettingsIcon,
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
}: {
  active: View;
  onSelect: (view: View) => void;
}) {
  return (
    <nav className="flex flex-col gap-1 px-3 py-6">
      <div className="flex items-center gap-4 px-3 pb-8">
        <img src={logo} alt="HostelDesk" className="h-12 w-12 rounded-lg" />
        <span className="text-display-md text-primary">HostelDesk</span>
      </div>
      {items.map((item) => {
        const selected = item.id === active;
        const Icon = item.icon;
        return (
          <button
            key={item.id}
            type="button"
            onClick={() => onSelect(item.id)}
            className={`flex items-center gap-3 rounded-full px-3 py-2.5 text-nav-link transition-colors ${
              selected
                ? 'bg-surface-strong text-ink'
                : 'text-muted hover:bg-surface-soft hover:text-ink'
            }`}
            aria-current={selected ? 'page' : undefined}
          >
            <Icon width={20} height={20} />
            {item.label}
          </button>
        );
      })}
    </nav>
  );
}
