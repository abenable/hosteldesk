import { useState } from 'react';
import * as Dialog from '@radix-ui/react-dialog';
import { Sidebar, type View } from './Sidebar.tsx';
import { Topbar } from './Topbar.tsx';
import { Button } from './ui/Button.tsx';
import { CheckInForm } from './CheckInForm.tsx';
import { Modal } from './ui/Modal.tsx';

const TITLES: Record<View, string> = {
  dashboard: 'Dashboard',
  rooms: 'Rooms',
  residents: 'Residents',
  invoices: 'Invoices',
  settings: 'Settings',
};

export function Layout({
  active,
  onSelect,
  hostelName,
  userName,
  children,
}: {
  active: View;
  onSelect: (view: View) => void;
  hostelName?: string;
  userName?: string;
  children: React.ReactNode;
}) {
  const [drawerOpen, setDrawerOpen] = useState(false);
  const [checkInOpen, setCheckInOpen] = useState(false);
  const [collapsed, setCollapsed] = useState(false);

  const handleSelect = (view: View) => {
    onSelect(view);
    setDrawerOpen(false);
  };

  return (
    <div className="flex h-screen overflow-hidden bg-canvas text-ink">
      <aside
        className={`hidden min-h-0 shrink-0 flex-col overflow-y-auto border-r border-hairline bg-canvas transition-[width] duration-200 md:flex ${
          collapsed ? 'w-20' : 'w-64'
        }`}
      >
        <Sidebar
          active={active}
          onSelect={onSelect}
          collapsed={collapsed}
          onToggleCollapse={() => setCollapsed((c) => !c)}
        />
      </aside>

      <Dialog.Root open={drawerOpen} onOpenChange={setDrawerOpen}>
        <Dialog.Portal>
          <Dialog.Overlay className="fixed inset-0 z-40 bg-scrim/40 backdrop-blur-sm" />
          <Dialog.Content className="fixed left-0 top-0 z-50 h-full w-72 bg-canvas p-2 shadow-card focus:outline-none">
            <Dialog.Close
              className="absolute right-3 top-3 rounded-full p-1 text-muted hover:bg-surface-soft hover:text-ink"
              aria-label="Close menu"
            />
            <Sidebar active={active} onSelect={handleSelect} />
          </Dialog.Content>
        </Dialog.Portal>
      </Dialog.Root>

      <div className="flex min-w-0 min-h-0 flex-1 flex-col">
        <Topbar
          title={TITLES[active]}
          hostelName={hostelName}
          userName={userName}
          onMenu={() => setDrawerOpen(true)}
          onCheckIn={active === 'dashboard' ? undefined : () => setCheckInOpen(true)}
          onSelectView={onSelect}
        />
        <main className="min-h-0 flex-1 overflow-auto px-4 py-6 md:px-8">
          <div className="mx-auto max-w-6xl">{children}</div>
        </main>
      </div>

      <Modal
        open={checkInOpen}
        onOpenChange={setCheckInOpen}
        title="Check in resident"
        description="Assign a vacant room to a new resident."
      >
        <CheckInForm onSuccess={() => setCheckInOpen(false)} />
        <div className="flex justify-end">
          <Button variant="tertiary" onClick={() => setCheckInOpen(false)}>
            Close
          </Button>
        </div>
      </Modal>
    </div>
  );
}
