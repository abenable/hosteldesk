import * as React from 'react'
import { Slot } from '@radix-ui/react-slot'
import * as Dialog from '@radix-ui/react-dialog'
import { cn } from '@/lib/utils'
import { useIsMobile } from '@/hooks/use-mobile'
import { SidebarContext, useSidebar } from '@/hooks/use-sidebar'

const SIDEBAR_WIDTH = '16rem'
const SIDEBAR_WIDTH_MOBILE = '18rem'
const SIDEBAR_WIDTH_ICON = '4rem'
const HEADER_HEIGHT = '3.5rem'

export function SidebarProvider({
  children,
  defaultOpen = true,
}: {
  children: React.ReactNode
  defaultOpen?: boolean
}) {
  const isMobile = useIsMobile()
  const [openMobile, setOpenMobile] = React.useState(false)
  const [open, setOpen] = React.useState(defaultOpen)

  const state = open ? 'expanded' : 'collapsed'
  const toggleSidebar = React.useCallback(() => {
    if (isMobile) {
      setOpenMobile((o) => !o)
    } else {
      setOpen((o) => !o)
    }
  }, [isMobile])

  return (
    <SidebarContext.Provider
      value={{ state, open, setOpen, openMobile, setOpenMobile, isMobile, toggleSidebar }}
    >
      <div
        className="group flex h-screen w-full overflow-hidden bg-surface-soft text-ink"
        data-collapsible={state === 'collapsed' ? 'offcanvas' : ''}
        style={{ '--sidebar-width': SIDEBAR_WIDTH, '--sidebar-width-icon': SIDEBAR_WIDTH_ICON, '--sidebar-width-mobile': SIDEBAR_WIDTH_MOBILE, '--header-height': HEADER_HEIGHT } as React.CSSProperties}
      >
        {children}
      </div>
    </SidebarContext.Provider>
  )
}

export function Sidebar({
  collapsible = 'offcanvas',
  variant = 'sidebar',
  className,
  children,
  ...props
}: React.ComponentProps<'div'> & {
  collapsible?: 'offcanvas' | 'icon' | 'none'
  variant?: 'sidebar' | 'floating' | 'inset'
}) {
  const { isMobile, openMobile, setOpenMobile, state } = useSidebar()

  if (collapsible === 'none') {
    return (
      <div
        className={cn('flex h-full flex-col bg-sidebar text-sidebar-foreground', className)}
        {...props}
      >
        {children}
      </div>
    )
  }

  if (isMobile) {
    return (
      <Dialog.Root open={openMobile} onOpenChange={setOpenMobile}>
        <Dialog.Portal>
          <Dialog.Overlay className="fixed inset-0 z-50 bg-scrim/50 backdrop-blur-sm data-[state=open]:animate-in data-[state=closed]:animate-out data-[state=closed]:fade-out-0 data-[state=open]:fade-in-0" />
          <Dialog.Content
            className={cn(
              'fixed inset-y-0 left-0 z-50 flex h-full w-[var(--sidebar-width-mobile)] flex-col bg-sidebar text-sidebar-foreground shadow-card focus:outline-none data-[state=open]:animate-in data-[state=closed]:animate-out data-[state=closed]:slide-out-to-left data-[state=open]:slide-in-from-left',
              className
            )}
            {...props}
          >
            {children}
          </Dialog.Content>
        </Dialog.Portal>
      </Dialog.Root>
    )
  }

  const isIcon = collapsible === 'icon'
  const collapsed = state === 'collapsed'

  return (
    <div
      className={cn(
        'group/sidebar relative hidden h-full flex-col bg-sidebar text-sidebar-foreground transition-[width] duration-200 ease-in-out md:flex',
        variant === 'inset' && 'bg-surface-soft',
        collapsed && collapsible === 'offcanvas' && 'hidden md:hidden',
        collapsed && isIcon && 'overflow-hidden',
        className
      )}
      data-state={state}
      data-collapsible={isIcon && collapsed ? 'icon' : ''}
      style={{ width: `var(${collapsed && isIcon ? '--sidebar-width-icon' : '--sidebar-width'})` }}
      {...props}
    >
      {children}
    </div>
  )
}

export function SidebarHeader({ className, ...props }: React.ComponentProps<'div'>) {
  return <div className={cn('flex flex-col gap-2 p-4', className)} {...props} />
}

export function SidebarContent({ className, ...props }: React.ComponentProps<'div'>) {
  return (
    <div
      className={cn('flex flex-1 flex-col gap-2 overflow-y-auto px-4 pb-4', className)}
      {...props}
    />
  )
}

export function SidebarFooter({ className, ...props }: React.ComponentProps<'div'>) {
  return <div className={cn('flex flex-col gap-2 p-4', className)} {...props} />
}

export function SidebarMenu({ className, ...props }: React.ComponentProps<'ul'>) {
  return <ul className={cn('flex w-full min-w-0 flex-col gap-1', className)} {...props} />
}

export function SidebarMenuItem({ className, ...props }: React.ComponentProps<'li'>) {
  return <li className={cn('group/menu-item relative', className)} {...props} />
}

const sidebarMenuButtonVariants = {
  default: 'hover:bg-sidebar-accent hover:text-sidebar-accent-foreground',
  outline:
    'bg-canvas shadow-[0_0_0_1px_hsl(240_5%_90%)] hover:bg-sidebar-accent hover:text-sidebar-accent-foreground hover:shadow-[0_0_0_1px_hsl(240_5%_90%)]',
}

const sidebarMenuButtonSizes = {
  default: 'h-8 text-body-sm',
  sm: 'h-7 text-caption-sm',
  lg: 'h-12 text-body-sm',
}

export function SidebarMenuButton({
  asChild = false,
  isActive = false,
  variant = 'default',
  size = 'default',
  className,
  ...props
}: React.ComponentProps<'button'> & {
  asChild?: boolean
  isActive?: boolean
  variant?: keyof typeof sidebarMenuButtonVariants
  size?: keyof typeof sidebarMenuButtonSizes
}) {
  const Comp = asChild ? Slot : 'button'
  return (
    <Comp
      className={cn(
        'flex w-full items-center gap-2 overflow-hidden rounded-full px-2 font-medium text-sidebar-foreground outline-none transition-colors duration-200 ease-out',
        sidebarMenuButtonVariants[variant],
        sidebarMenuButtonSizes[size],
        isActive &&
          'bg-primary/10 text-primary font-semibold hover:bg-primary/10 hover:text-primary',
        'disabled:pointer-events-none disabled:opacity-50',
        className
      )}
      {...props}
    />
  )
}

export function SidebarInset({ className, ...props }: React.ComponentProps<'main'>) {
  return (
    <main
      className={cn(
        'flex h-full min-w-0 flex-1 flex-col overflow-hidden bg-canvas',
        className
      )}
      {...props}
    />
  )
}

export function SidebarTrigger({
  className,
  ...props
}: React.ComponentProps<'button'>) {
  const { toggleSidebar } = useSidebar()
  return (
    <button
      type="button"
      onClick={toggleSidebar}
      className={cn(
        'flex h-7 w-7 items-center justify-center rounded-md text-muted hover:bg-surface-soft hover:text-ink transition-colors',
        className
      )}
      {...props}
    >
      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
        <rect width="18" height="18" x="3" y="3" rx="2" />
        <path d="M9 3v18" />
      </svg>
      <span className="sr-only">Toggle Sidebar</span>
    </button>
  )
}

