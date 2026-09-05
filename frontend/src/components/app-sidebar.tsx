import logo from '@/assets/logo.png'
import { cn } from '@/lib/utils'
import { NavMain } from '@/components/nav-main'
import { NavSecondary } from '@/components/nav-secondary'
import { NavUser } from '@/components/nav-user'
import {
  Sidebar,
  SidebarContent,
  SidebarFooter,
  SidebarHeader,
  SidebarMenu,
  SidebarMenuButton,
  SidebarMenuItem,
} from '@/components/ui/sidebar'
import { useSidebar } from '@/hooks/use-sidebar'
import type { View } from './types'

export function AppSidebar({
  active,
  onSelect,
  userName,
  userEmail,
  onSignOut,
}: {
  active: View
  onSelect: (v: View) => void
  userName: string
  userEmail: string
  onSignOut: () => Promise<void>
}) {
  const { state } = useSidebar()
  return (
    <Sidebar collapsible="icon" variant="inset">
      <SidebarHeader>
        <SidebarMenu>
          <SidebarMenuItem>
            <SidebarMenuButton
              asChild
              className={cn(
                'data-[slot=sidebar-menu-button]:!p-1.5',
                state === 'collapsed' && 'justify-center px-0'
              )}
            >
              <a href="#">
                <img src={logo} alt="HostelDesk" className="h-7 w-7 shrink-0 rounded-md" />
                {state === 'expanded' && (
                  <span className="text-base font-semibold text-primary">HostelDesk</span>
                )}
              </a>
            </SidebarMenuButton>
          </SidebarMenuItem>
        </SidebarMenu>
      </SidebarHeader>
      <SidebarContent>
        <NavMain active={active} onSelect={onSelect} />
        <NavSecondary active={active} onSelect={onSelect} className="mt-auto" />
      </SidebarContent>
      <SidebarFooter>
        <NavUser
          name={userName}
          email={userEmail}
          onSelectView={onSelect}
          onSignOut={onSignOut}
        />
      </SidebarFooter>
    </Sidebar>
  )
}
