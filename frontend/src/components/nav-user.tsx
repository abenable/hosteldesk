import { cn } from '@/lib/utils'
import {
  Avatar,
  AvatarFallback,
} from '@/components/ui/avatar'
import { Dropdown, DropdownItem } from '@/components/ui/Dropdown'
import {
  DotsVerticalIcon,
  LogoutIcon,
  SettingsIcon,
  UserCircleIcon,
} from './ui/icons.tsx'
import {
  SidebarMenu,
  SidebarMenuButton,
  SidebarMenuItem,
} from '@/components/ui/sidebar'
import { useSidebar } from '@/hooks/use-sidebar'
import type { View } from './types'

export function NavUser({
  name,
  email,
  onSelectView,
  onSignOut,
}: {
  name: string
  email: string
  onSelectView: (v: View) => void
  onSignOut: () => Promise<void>
}) {
  const { state } = useSidebar()
  const initials = name.split(' ').map((w) => w[0]).slice(0, 2).join('').toUpperCase()
  const collapsed = state === 'collapsed'

  return (
    <SidebarMenu>
      <SidebarMenuItem>
        <Dropdown
          trigger={
            <SidebarMenuButton
              size="lg"
              className={cn(
                'bg-hairline data-[state=open]:bg-sidebar-accent data-[state=open]:text-sidebar-accent-foreground',
                collapsed && 'justify-center px-0'
              )}
            >
              <Avatar className="h-8 w-8 rounded-lg">
                <AvatarFallback className="rounded-lg">{initials}</AvatarFallback>
              </Avatar>
              {!collapsed && (
                <div className="grid flex-1 text-left text-sm leading-tight">
                  <span className="truncate font-medium">{name}</span>
                  <span className="truncate text-xs text-muted">{email}</span>
                </div>
              )}
              {!collapsed && <DotsVerticalIcon className="ml-auto size-4" strokeWidth={3} />}
            </SidebarMenuButton>
          }
        >
          <div className="flex items-center gap-2 px-3 py-2 text-left">
            <Avatar className="h-8 w-8 rounded-lg">
              <AvatarFallback className="rounded-lg">{initials}</AvatarFallback>
            </Avatar>
            <div className="grid flex-1 text-left text-sm leading-tight">
              <span className="truncate font-medium">{name}</span>
              <span className="truncate text-xs text-muted">{email}</span>
            </div>
          </div>
          <DropdownItem onSelect={() => onSelectView('settings')}>
            <UserCircleIcon className="size-4" /> Account
          </DropdownItem>
          <DropdownItem onSelect={() => onSelectView('settings')}>
            <SettingsIcon className="size-4" /> Settings
          </DropdownItem>
          <DropdownItem danger onSelect={onSignOut}>
            <LogoutIcon className="size-4" /> Log out
          </DropdownItem>
        </Dropdown>
      </SidebarMenuItem>
    </SidebarMenu>
  )
}
