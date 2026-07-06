import { useState } from 'react';
import { Sidebar } from './Sidebar.tsx';

export type View = 'dashboard' | 'rooms' | 'residents' | 'settings';

export function Layout({
  active,
  onSelect,
  children,
}: {
  active: View;
  onSelect: (view: View) => void;
  children: React.ReactNode;
}) {
  const [open, setOpen] = useState(false);

  return (
    <div className="flex min-h-screen bg-canvas text-ink">
      {/* Desktop sidebar */}
      <aside className="hidden w-60 shrink-0 flex-col border-r border-hairline bg-canvas md:flex">
        <Sidebar active={active} onSelect={onSelect} />
      </aside>

      {/* Mobile header */}
      <div className="fixed left-0 right-0 top-0 z-20 flex items-center justify-between border-b border-hairline bg-canvas px-4 py-3 md:hidden">
        <span className="text-title-md text-primary">HostelDesk</span>
        <button
          type="button"
          onClick={() => setOpen((s) => !s)}
          className="rounded-sm border border-hairline px-3 py-1.5 text-button-sm text-ink"
          aria-label="Toggle menu"
          aria-expanded={open}
        >
          Menu
        </button>
      </div>

      {open && (
        <div className="fixed left-0 right-0 top-12 z-20 border-b border-hairline bg-canvas md:hidden">
          <Sidebar
            active={active}
            onSelect={(view) => {
              onSelect(view);
              setOpen(false);
            }}
          />
        </div>
      )}

      <main className="flex-1 overflow-auto px-4 pb-8 pt-16 md:pt-8">
        <div className="mx-auto max-w-6xl">
          <h1 className="mb-6 text-display-lg capitalize">{active}</h1>
          {children}
        </div>
      </main>
    </div>
  );
}
