import { useEffect, useRef, useState, type KeyboardEvent, type ReactNode } from 'react';
import { cx } from './cx';

/** One entry of a {@link Dropdown}. */
export interface MenuItem {
  label: ReactNode;
  onSelect: () => void;
  /** Red, for destructive actions. */
  danger?: boolean;
  disabled?: boolean;
}

/** Props of {@link Dropdown}. */
export interface DropdownProps {
  /** Content of the trigger button (text, avatar, or an `<Icon name="ellipsis-vertical" />`). */
  trigger: ReactNode;
  /** Accessible name of the trigger, e.g. "Actions for room 12". */
  label: string;
  items: MenuItem[];
  /** Non-interactive block at the top of the menu (e.g. the signed-in user). */
  header?: ReactNode;
  /** Classes for the trigger; defaults to a quiet icon button. */
  triggerClassName?: string;
}

/**
 * Menu button (HyperUI dropdown, "Divided"). Opens below the trigger (above it
 * near the bottom of the window), aligned to its end edge; closes on Escape, outside click, scroll or a choice. The
 * menu is `position: fixed`, so it is never clipped by a scrolling table.
 *
 * ```tsx
 * <Dropdown label={`Actions for ${r.full_name}`} trigger={<Icon name="ellipsis-vertical" />}
 *   items={[{ label: 'Check out', danger: true, onSelect: () => checkOut(r) }]} />
 * ```
 */
export function Dropdown({ trigger, label, items, header, triggerClassName }: DropdownProps) {
  const [at, setAt] = useState<{ top?: number; bottom?: number; right: number } | null>(null);
  const button = useRef<HTMLButtonElement>(null);
  const menu = useRef<HTMLDivElement>(null);
  const open = at !== null;

  useEffect(() => {
    if (!open) return;
    menu.current?.querySelector<HTMLElement>('[role="menuitem"]:not(:disabled)')?.focus();
    const away = (event: Event) => {
      const target = event.target as Node;
      if (!menu.current?.contains(target) && !button.current?.contains(target)) setAt(null);
    };
    const close = () => setAt(null);
    document.addEventListener('mousedown', away);
    window.addEventListener('resize', close);
    window.addEventListener('scroll', close, true);
    return () => {
      document.removeEventListener('mousedown', away);
      window.removeEventListener('resize', close);
      window.removeEventListener('scroll', close, true);
    };
  }, [open]);

  const toggle = () => {
    const rect = button.current?.getBoundingClientRect();
    if (open || !rect) return setAt(null);
    const right = window.innerWidth - rect.right;
    // Near the bottom of the window (e.g. the sidebar user card) the menu opens upwards.
    setAt(
      window.innerHeight - rect.bottom < 240
        ? { bottom: window.innerHeight - rect.top + 8, right }
        : { top: rect.bottom + 8, right },
    );
  };

  const onMenuKey = (event: KeyboardEvent) => {
    const entries = [
      ...(menu.current?.querySelectorAll<HTMLElement>('[role="menuitem"]:not(:disabled)') ?? []),
    ];
    const index = entries.indexOf(document.activeElement as HTMLElement);
    if (event.key === 'Escape' || event.key === 'Tab') {
      setAt(null);
      button.current?.focus();
    } else if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
      event.preventDefault();
      const step = event.key === 'ArrowDown' ? 1 : -1;
      entries[(index + step + entries.length) % entries.length]?.focus();
    }
  };

  return (
    <>
      <button
        ref={button}
        type="button"
        aria-label={label}
        aria-haspopup="menu"
        aria-expanded={open}
        onClick={toggle}
        className={
          triggerClassName ??
          'rounded-full p-1.5 text-gray-600 transition-colors hover:bg-gray-100 hover:text-gray-900 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-primary-900'
        }
      >
        {trigger}
      </button>
      {at && (
        <div
          ref={menu}
          role="menu"
          tabIndex={-1}
          aria-label={label}
          onKeyDown={onMenuKey}
          style={at}
          className="fixed z-40 w-56 divide-y divide-gray-200 overflow-hidden rounded-xl border border-gray-200 bg-white shadow-lg"
        >
          {header && <div className="px-3 py-2">{header}</div>}
          <div>
            {items.map((item, i) => (
              <button
                key={i}
                type="button"
                role="menuitem"
                disabled={item.disabled}
                onClick={() => {
                  setAt(null);
                  button.current?.focus();
                  item.onSelect();
                }}
                className={cx(
                  'block w-full px-3 py-2 text-start text-sm font-medium transition-colors focus-visible:outline-none disabled:cursor-not-allowed disabled:opacity-50',
                  item.danger
                    ? 'text-red-700 hover:bg-red-50 focus-visible:bg-red-50'
                    : 'text-gray-700 hover:bg-gray-50 hover:text-gray-900 focus-visible:bg-gray-50',
                )}
              >
                {item.label}
              </button>
            ))}
          </div>
        </div>
      )}
    </>
  );
}
