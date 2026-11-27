import type { ComponentProps } from 'react';
import { cx } from './cx';
import { Icon, type IconName } from './Icon';

const VARIANTS = {
  primary:
    'bg-primary-900 text-white shadow-xs hover:bg-primary-800 focus-visible:outline-primary-900',
  secondary:
    'bg-white text-gray-900 shadow-xs ring-1 ring-gray-300 ring-inset hover:bg-gray-50 focus-visible:outline-primary-900',
  ghost:
    'text-gray-700 hover:bg-gray-100 hover:text-gray-900 focus-visible:outline-gray-900',
  danger:
    'bg-red-600 text-white hover:bg-red-700 focus-visible:outline-red-600',
};

const SIZES = {
  sm: 'gap-1.5 px-3 py-1.5 text-sm/none',
  md: 'gap-2 px-4 py-2 text-sm',
};

/** Props of {@link Button}. */
export interface ButtonProps extends ComponentProps<'button'> {
  variant?: keyof typeof VARIANTS;
  size?: keyof typeof SIZES;
  /** Shows a spinner and disables the button. Pass `action.busy` here so a write is never sent twice. */
  loading?: boolean;
  /** Heroicon shown before the label. */
  icon?: IconName;
}

/**
 * Button (HyperUI modal/toast buttons). `type` defaults to `"button"`.
 * `primary` is the solid brand action (one per view); `secondary` is the
 * rounded outline button for page-header actions and Cancel.
 *
 * ```tsx
 * <Button icon="plus" onClick={open}>Add room</Button>
 * <Button type="submit" loading={save.busy}>Save</Button>
 * <Button variant="secondary" size="sm">Cancel</Button>
 * <Button variant="danger" onClick={remove}>Remove</Button>
 * ```
 */
export function Button({
  variant = 'primary',
  size = 'md',
  loading = false,
  icon,
  type = 'button',
  disabled,
  className,
  children,
  ...props
}: ButtonProps) {
  return (
    <button
      type={type}
      disabled={disabled || loading}
      aria-busy={loading || undefined}
      className={cx(
        'inline-flex shrink-0 items-center justify-center rounded-lg font-medium whitespace-nowrap transition-colors focus-visible:outline-2 focus-visible:outline-offset-2 disabled:cursor-not-allowed disabled:opacity-50',
        VARIANTS[variant],
        SIZES[size],
        className,
      )}
      {...props}
    >
      {loading ? (
        <svg
          aria-hidden="true"
          xmlns="http://www.w3.org/2000/svg"
          fill="none"
          viewBox="0 0 24 24"
          strokeWidth={2}
          stroke="currentColor"
          className="size-4 animate-spin motion-reduce:animate-none"
        >
          <path strokeLinecap="round" strokeLinejoin="round" d="M12 3a9 9 0 1 0 9 9" />
        </svg>
      ) : (
        icon && <Icon name={icon} className="-ms-0.5 size-4" />
      )}
      {children}
    </button>
  );
}
