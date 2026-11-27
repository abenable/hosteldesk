import type { ReactNode } from 'react';
import { cx } from './cx';
import { Icon, type IconName } from './Icon';

const TONES = {
  neutral: 'bg-gray-100 text-gray-700 ring-gray-500/15',
  success: 'bg-green-50 text-green-700 ring-green-600/20',
  warning: 'bg-amber-50 text-amber-800 ring-amber-600/20',
  danger: 'bg-red-50 text-red-700 ring-red-600/15',
  info: 'bg-primary-100 text-primary-800 ring-primary-900/10',
};

/** Badge colour. success = paid/available, warning = partial, danger = owing/full, info = brand. */
export type BadgeTone = keyof typeof TONES;

/**
 * Soft tinted status pill with an optional small icon (HyperUI badges, "Themed with icon").
 *
 * ```tsx
 * <Badge tone="success" icon="check-circle">Paid</Badge>
 * <Badge tone={isOwing(r.balance) ? 'danger' : 'success'}>{isOwing(r.balance) ? 'Owing' : 'Paid'}</Badge>
 * ```
 */
export function Badge({
  tone = 'neutral',
  icon,
  children,
}: {
  tone?: BadgeTone;
  icon?: IconName;
  children: ReactNode;
}) {
  return (
    <span
      className={cx(
        'inline-flex items-center gap-1 rounded-full px-2 py-0.5 text-xs font-medium whitespace-nowrap ring-1 ring-inset',
        TONES[tone],
      )}
    >
      {icon && <Icon name={icon} className="-ms-0.5 size-3.5" />}
      {children}
    </span>
  );
}
