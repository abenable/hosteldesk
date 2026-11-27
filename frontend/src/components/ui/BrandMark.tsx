import { cx } from './cx';

const SIZES = { sm: 'size-9 rounded-lg text-base', lg: 'size-14 rounded-2xl text-2xl' };

/** The HostelDesk mark: a black tile with a white "H" (the monochrome stand-in for the app icon). */
export function BrandMark({ size = 'sm' }: { size?: keyof typeof SIZES }) {
  return (
    <span
      aria-hidden="true"
      className={cx(
        'inline-flex shrink-0 items-center justify-center bg-gray-950 font-semibold text-white shadow-xs',
        SIZES[size],
      )}
    >
      H
    </span>
  );
}
