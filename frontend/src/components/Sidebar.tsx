type View = 'dashboard' | 'rooms' | 'residents' | 'settings';

const items: { id: View; label: string }[] = [
  { id: 'dashboard', label: 'Dashboard' },
  { id: 'rooms', label: 'Rooms' },
  { id: 'residents', label: 'Residents' },
  { id: 'settings', label: 'Settings' },
];

export function Sidebar({
  active,
  onSelect,
}: {
  active: View;
  onSelect: (view: View) => void;
}) {
  return (
    <nav className="flex flex-col gap-1">
      <div className="px-4 py-6 text-display-sm text-primary">HostelDesk</div>
      {items.map((item) => {
        const selected = item.id === active;
        return (
          <button
            key={item.id}
            type="button"
            onClick={() => onSelect(item.id)}
            className={`border-b-2 px-4 py-3 text-left text-nav-link transition-colors ${
              selected
                ? 'border-ink text-ink'
                : 'border-transparent text-muted hover:text-ink'
            }`}
            aria-current={selected ? 'page' : undefined}
          >
            {item.label}
          </button>
        );
      })}
    </nav>
  );
}
