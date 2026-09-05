import { SidebarTrigger } from '@/components/ui/sidebar'
import { Button } from './ui/Button.tsx'
import type { View } from './types'

const TITLES: Record<View, string> = {
  dashboard: 'Dashboard',
  rooms: 'Rooms',
  residents: 'Residents',
  invoices: 'Payments',
  settings: 'Settings',
}

export function SiteHeader({
  active,
  onCheckIn,
}: {
  active: View
  onCheckIn?: () => void
}) {
  const showCheckIn = active !== 'dashboard' && active !== 'settings'

  return (
    <header className="flex h-14 shrink-0 items-center justify-between gap-4 border-b border-hairline bg-canvas px-4 lg:px-6">
      <div className="flex items-center gap-3">
        <SidebarTrigger className="-ml-1" />
        <h1 className="text-title-md text-ink">{TITLES[active]}</h1>
      </div>

      {showCheckIn && (
        <Button variant="pill" size="sm" onClick={onCheckIn} className="hidden sm:inline-flex">
          New Resident
        </Button>
      )}
    </header>
  )
}
