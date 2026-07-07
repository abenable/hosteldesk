import { Dropdown, DropdownItem } from './ui/Dropdown.tsx';
import { Button } from './ui/Button.tsx';
import { MenuIcon, PlusIcon, LogoutIcon, SettingsIcon } from './ui/icons.tsx';
import type { View } from './Sidebar.tsx';

type Props = {
  title: string;
  hostelName?: string;
  userName?: string;
  onMenu?: () => void;
  onCheckIn?: () => void;
  onSelectView: (v: View) => void;
};

export function Topbar({
  title,
  hostelName,
  userName,
  onMenu,
  onCheckIn,
  onSelectView,
}: Props) {
  return (
    <header className="z-20 flex shrink-0 items-center justify-between gap-4 border-b border-hairline bg-canvas/95 px-4 py-3 backdrop-blur md:px-8">
      <div className="flex items-center gap-3">
        <button
          type="button"
          onClick={onMenu}
          className="rounded-full p-2 text-ink hover:bg-surface-soft md:hidden"
          aria-label="Open menu"
        >
          <MenuIcon width={20} height={20} />
        </button>
        <h1 className="text-display-sm text-ink">{title}</h1>
      </div>

      <div className="flex items-center gap-2 md:gap-3">
        {onCheckIn && (
          <Button variant="pill" onClick={onCheckIn} className="hidden sm:inline-flex">
            <PlusIcon width={16} height={16} />
            Check in
          </Button>
        )}
        {hostelName && (
          <span className="hidden text-caption text-muted lg:inline">{hostelName}</span>
        )}
        <Dropdown
          trigger={
            <button
              type="button"
              className="flex items-center gap-2 rounded-full border border-hairline px-2.5 py-1.5 text-body-sm text-ink hover:bg-surface-soft"
            >
              <span className="flex h-7 w-7 items-center justify-center rounded-full bg-primary text-on-primary text-caption-sm">
                {(userName ?? '?').slice(0, 1).toUpperCase()}
              </span>
            </button>
          }
        >
          <div className="px-3 py-2">
            <p className="text-body-sm text-ink">{userName ?? 'Staff'}</p>
            {hostelName && <p className="text-caption-sm text-muted">{hostelName}</p>}
          </div>
          <div className="my-1 h-px bg-hairline" />
          <DropdownItem onSelect={() => onSelectView('settings')}>
            <SettingsIcon width={16} height={16} /> Settings
          </DropdownItem>
          <DropdownItem danger onSelect={() => window.location.reload()}>
            <LogoutIcon width={16} height={16} /> Sign out
          </DropdownItem>
        </Dropdown>
      </div>
    </header>
  );
}
