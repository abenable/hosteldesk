import type { ReactNode } from 'react';
import * as DropdownMenu from '@radix-ui/react-dropdown-menu';

type ItemProps = {
  children: ReactNode;
  onSelect?: () => void;
  danger?: boolean;
};

export function DropdownItem({ children, onSelect, danger }: ItemProps) {
  return (
    <DropdownMenu.Item
      onSelect={onSelect}
      className={`flex cursor-pointer items-center gap-2 rounded-xs px-3 py-2 text-body-sm outline-none focus:bg-surface-soft ${
        danger ? 'text-error-text' : 'text-ink'
      }`}
    >
      {children}
    </DropdownMenu.Item>
  );
}

type Props = {
  trigger: ReactNode;
  children: ReactNode;
  align?: 'start' | 'center' | 'end';
};

export function Dropdown({ trigger, children, align = 'end' }: Props) {
  return (
    <DropdownMenu.Root>
      <DropdownMenu.Trigger asChild>{trigger}</DropdownMenu.Trigger>
      <DropdownMenu.Portal>
        <DropdownMenu.Content
          align={align}
          sideOffset={8}
          className="z-50 min-w-[12rem] rounded-md border border-hairline bg-canvas p-1 shadow-card focus:outline-none"
        >
          {children}
          <DropdownMenu.Arrow className="fill-canvas" />
        </DropdownMenu.Content>
      </DropdownMenu.Portal>
    </DropdownMenu.Root>
  );
}
