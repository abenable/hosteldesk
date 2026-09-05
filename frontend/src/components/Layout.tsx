import { useState } from 'react';
import { SidebarProvider, SidebarInset } from '@/components/ui/sidebar';
import { AppSidebar } from '@/components/app-sidebar';
import { SiteHeader } from '@/components/site-header';
import { CheckInForm } from './CheckInForm.tsx';
import { Modal } from './ui/Modal.tsx';
import type { View } from './types';

export function Layout({
  active,
  onSelect,
  userName,
  userEmail,
  onSignOut,
  children,
}: {
  active: View;
  onSelect: (view: View) => void;
  userName?: string;
  userEmail?: string;
  onSignOut: () => Promise<void>;
  children: React.ReactNode;
}) {
  const [checkInOpen, setCheckInOpen] = useState(false);

  return (
    <SidebarProvider>
      <AppSidebar
        active={active}
        onSelect={onSelect}
        userName={userName ?? 'Staff'}
        userEmail={userEmail ?? 'staff@hosteldesk'}
        onSignOut={onSignOut}
      />
      <SidebarInset>
        <SiteHeader active={active} onCheckIn={() => setCheckInOpen(true)} />
        <div className="flex flex-1 flex-col overflow-y-auto">
          <div className="@container/main flex flex-1 flex-col gap-4 py-4 md:gap-6 md:py-6">
            <div className="px-4 lg:px-6">
              {children}
            </div>
          </div>
        </div>
      </SidebarInset>

      <Modal
        open={checkInOpen}
        onOpenChange={setCheckInOpen}
        title="Check in resident"
        description="Assign a vacant room to a new resident."
      >
        {checkInOpen && <CheckInForm onSuccess={() => setCheckInOpen(false)} />}
      </Modal>
    </SidebarProvider>
  );
}
