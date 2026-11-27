import { useId, useRef, type KeyboardEvent, type ReactNode } from 'react';
import { cx } from './cx';

/** One tab of {@link Tabs}. */
export interface TabItem<V extends string> {
  value: V;
  label: ReactNode;
  /** Muted count after the label: "Current 32". */
  count?: number;
}

/** Props of {@link Tabs}. */
export interface TabsProps<V extends string> {
  items: TabItem<V>[];
  value: V;
  onChange: (value: V) => void;
  /** Accessible name of the tab list, e.g. "Resident filter". */
  label: string;
  /** `segmented` (default): grey track, white active segment. `underline` for long section lists. */
  variant?: 'segmented' | 'underline';
  /** Segmented only: stretch to the container's width with equal segments. */
  full?: boolean;
}

const STYLES = {
  segmented: {
    list: 'gap-1 rounded-full bg-gray-100 p-1',
    tab: 'group flex items-center justify-center gap-1.5 rounded-full px-3.5 py-1.5 text-sm font-medium whitespace-nowrap text-gray-600 transition-colors hover:text-gray-900 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-primary-900 aria-selected:bg-white aria-selected:text-gray-900 aria-selected:shadow-xs',
  },
  underline: {
    list: 'w-full gap-6 border-b border-gray-200',
    tab: 'group -mb-px flex shrink-0 items-center gap-1.5 border-b-2 border-transparent px-1 py-3 text-sm font-medium text-gray-600 hover:border-gray-300 hover:text-gray-900 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-primary-900 aria-selected:border-primary-900 aria-selected:text-primary-900',
  },
};

/**
 * Tab bar (HyperUI tabs, "Segmented" / "Underline with counts"), with
 * arrow-key, Home and End navigation. Render the selected panel yourself.
 *
 * ```tsx
 * <Tabs label="Residents" value={tab} onChange={setTab}
 *   items={[{ value: 'current', label: 'Current', count: 32 }, { value: 'past', label: 'Past', count: 140 }]} />
 * ```
 */
export function Tabs<V extends string>({
  items,
  value,
  onChange,
  label,
  variant = 'segmented',
  full = false,
}: TabsProps<V>) {
  const id = useId();
  const list = useRef<HTMLDivElement>(null);
  const style = STYLES[variant];

  const onKeyDown = (event: KeyboardEvent, index: number) => {
    const keys: Record<string, number> = {
      ArrowRight: (index + 1) % items.length,
      ArrowLeft: (index - 1 + items.length) % items.length,
      Home: 0,
      End: items.length - 1,
    };
    const next = keys[event.key];
    const item = next === undefined ? undefined : items[next];
    if (next === undefined || !item) return;
    event.preventDefault();
    onChange(item.value);
    list.current?.querySelectorAll<HTMLElement>('[role="tab"]')[next]?.focus();
  };

  return (
    <div
      ref={list}
      role="tablist"
      aria-label={label}
      className={cx(
        style.list,
        variant === 'segmented' && full ? 'grid auto-cols-fr grid-flow-col' : 'inline-flex max-w-full overflow-x-auto',
      )}
    >
      {items.map((item, index) => {
        const selected = item.value === value;
        return (
          <button
            key={item.value}
            type="button"
            role="tab"
            id={`${id}-${item.value}`}
            aria-selected={selected}
            tabIndex={selected ? 0 : -1}
            onClick={() => onChange(item.value)}
            onKeyDown={(e) => onKeyDown(e, index)}
            className={style.tab}
          >
            {item.label}
            {item.count !== undefined && (
              <span className="font-normal text-gray-500 tabular-nums group-aria-selected:text-gray-600">
                {item.count}
              </span>
            )}
          </button>
        );
      })}
    </div>
  );
}
