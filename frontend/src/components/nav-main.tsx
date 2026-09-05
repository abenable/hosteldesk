import {
  DashboardIcon,
  RoomsIcon,
  ResidentsIcon,
  InvoiceIcon,
} from './ui/icons.tsx'
import type { SVGProps, ReactNode } from 'react'
import {
  SidebarMenu,
  SidebarMenuItem,
  SidebarMenuButton,
} from '@/components/ui/sidebar'
import { useSidebar } from '@/hooks/use-sidebar'
import type { View } from './types'

type SidebarIcon = (props: SVGProps<SVGSVGElement>) => ReactNode

const items: { id: View; label: string; icon: SidebarIcon }[] = [
  { id: 'dashboard', label: 'Dashboard', icon: DashboardIcon },
  { id: 'rooms', label: 'Rooms', icon: RoomsIcon },
  { id: 'residents', label: 'Residents', icon: ResidentsIcon },
  { id: 'invoices', label: 'Payments', icon: InvoiceIcon },
]

export function NavMain({
  active,
  onSelect,
}: {
  active: View
  onSelect: (v: View) => void
}) {
  const { state } = useSidebar()
  return (
    <SidebarMenu>
      {items.map((item) => (
        <SidebarMenuItem key={item.id}>
          <SidebarMenuButton
            isActive={active === item.id}
            onClick={() => onSelect(item.id)}
            className={state === 'collapsed' ? 'justify-center px-0' : undefined}
          >
            <item.icon className="size-5" />
            {state === 'expanded' && <span>{item.label}</span>}
          </SidebarMenuButton>
        </SidebarMenuItem>
      ))}
    </SidebarMenu>
  )
}
