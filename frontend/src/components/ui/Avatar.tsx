import { cx } from './cx';

// Grey tints (the theme is monochrome); the same name always gets the same one.
const TINTS = [
  'bg-gray-100 text-gray-700 ring-1 ring-gray-900/5 ring-inset',
  'bg-gray-200 text-gray-800',
  'bg-gray-300 text-gray-900',
  'bg-gray-800 text-white',
];

const SIZES = { sm: 'size-8 text-xs', md: 'size-10 text-sm', lg: 'size-14 text-lg' };

/** Up to two initials: "Jane Nakato Doe" → "JN". */
function initials(name: string): string {
  const parts = name.trim().split(/\s+/).filter(Boolean);
  return parts.slice(0, 2).map((part) => part[0]?.toUpperCase() ?? '').join('') || '?';
}

/**
 * Initials on a grey tint picked from the name (HyperUI avatars, "Initials").
 * Decorative: the name is always written next to it.
 *
 * ```tsx
 * <Avatar name={resident.full_name} size="sm" />
 * ```
 */
export function Avatar({ name, size = 'md' }: { name: string; size?: keyof typeof SIZES }) {
  let hash = 0;
  for (const char of name) hash = (hash * 31 + (char.codePointAt(0) ?? 0)) >>> 0;
  return (
    <span
      aria-hidden="true"
      className={cx(
        'inline-flex shrink-0 items-center justify-center rounded-full font-medium',
        SIZES[size],
        TINTS[hash % TINTS.length],
      )}
    >
      {initials(name)}
    </span>
  );
}
