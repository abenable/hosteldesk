import { cx } from './cx';

const SIZES = { sm: 'size-4', md: 'size-6', lg: 'size-8' };

/**
 * Spinner (HyperUI loaders, "Spinner" / "Spinner with text"). With `label` it
 * stacks the text under the spinner and centres both in a padded block.
 *
 * ```tsx
 * <Spinner label="Loading residents…" />
 * <Spinner size="sm" />
 * ```
 */
export function Spinner({ label, size = 'md' }: { label?: string; size?: keyof typeof SIZES }) {
  const svg = (
    <svg
      className={cx('animate-spin text-primary-900', SIZES[size], label && 'mx-auto')}
      xmlns="http://www.w3.org/2000/svg"
      fill="none"
      viewBox="0 0 24 24"
      aria-hidden="true"
    >
      <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4" />
      <path
        className="opacity-75"
        fill="currentColor"
        d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"
      />
    </svg>
  );
  if (!label)
    return (
      <output className="inline-flex">
        {svg}
        <span className="sr-only">Loading</span>
      </output>
    );
  return (
    <output className="block py-12 text-center">
      {svg}
      <span className="mt-4 block text-sm font-medium text-gray-700">{label}</span>
    </output>
  );
}

const WIDTHS = ['w-full', 'w-11/12', 'w-4/5', 'w-2/3'];

/**
 * Placeholder lines while a list or form loads (pulse, HyperUI loaders).
 *
 * ```tsx
 * if (loading) return <Card><Skeleton rows={6} /></Card>;
 * ```
 */
export function Skeleton({ rows = 3 }: { rows?: number }) {
  return (
    <output className="block animate-pulse space-y-3">
      {Array.from({ length: rows }, (_, i) => (
        <div
          key={i}
          className={cx('h-4 rounded-sm bg-gray-200', WIDTHS[i % WIDTHS.length])}
        />
      ))}
      <span className="sr-only">Loading</span>
    </output>
  );
}
