import { SettingsIcon } from './ui/icons.tsx'
import {
  SidebarMenu,
  SidebarMenuItem,
  SidebarMenuButton,
} from '@/components/ui/sidebar'
import { useSidebar } from '@/hooks/use-sidebar'
import type { View } from './types'

export function NavSecondary({
  active,
  onSelect,
  className,
}: {
  active: View
  onSelect: (v: View) => void
  className?: string
}) {
  const { state } = useSidebar()
  return (
    <SidebarMenu className={className}>
      <SidebarMenuItem>
        <SidebarMenuButton
          isActive={active === 'settings'}
          onClick={() => onSelect('settings')}
          className={state === 'collapsed' ? 'justify-center px-0' : undefined}
        >
          <SettingsIcon className="size-5" />
          {state === 'expanded' && <span>Settings</span>}
        </SidebarMenuButton>
      </SidebarMenuItem>
    </SidebarMenu>
  )
}
