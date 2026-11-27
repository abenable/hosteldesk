import type { ReactNode } from 'react';
import { Button } from './Button';
import { cx } from './cx';
import { Icon, type IconName } from './Icon';

const TONES: Record<AlertTone, { box: string; title: string; icon: IconName }> = {
  danger: {
    box: 'border-red-200 bg-red-50',
    title: 'text-red-800',
    icon: 'exclamation-circle',
  },
  warning: {
    box: 'border-amber-200 bg-amber-50',
    title: 'text-amber-900',
    icon: 'exclamation-triangle',
  },
  success: {
    box: 'border-green-200 bg-green-50',
    title: 'text-green-800',
    icon: 'check-circle',
  },
  info: {
    box: 'border-gray-200 bg-gray-50',
    title: 'text-gray-900',
    icon: 'information-circle',
  },
};

/** Alert colour: danger (errors), warning (amber notes), success, info (neutral grey notes). */
export type AlertTone = 'danger' | 'warning' | 'success' | 'info';

/** Props of {@link Alert}. */
export interface AlertProps {
  tone?: AlertTone;
  title: ReactNode;
  /** The message. */
  children?: ReactNode;
  /** Replaces the tone's icon, e.g. `"calendar-days"` for a stay note. */
  icon?: IconName;
  /** Shows a "Try again" button, e.g. `useLoad(...).reload`. */
  onRetry?: () => void;
  /** Spinner on the Try again button. */
  retrying?: boolean;
  /** Extra buttons next to Try again. */
  action?: ReactNode;
}

/**
 * Tinted callout (HyperUI toast layout as an inline box, tuned to Tabula's
 * notes): the error state of every data surface, and the info/note boxes of a
 * DetailPanel.
 *
 * ```tsx
 * if (error && !data) return <Alert title="Could not load rooms" onRetry={reload}>{error}</Alert>;
 * <Alert tone="info" icon="calendar-days" title="Checks out on 20 Dec">Semester 1 · 2026/2027</Alert>
 * <Alert tone="warning" title="Owes UGX 150,000">Last paid 2 Nov.</Alert>
 * ```
 */
export function Alert({ tone = 'danger', title, children, icon, onRetry, retrying, action }: AlertProps) {
  const style = TONES[tone];
  return (
    <div
      role={tone === 'danger' ? 'alert' : 'status'}
      className={cx('rounded-xl border p-4', style.box)}
    >
      <div className="flex items-start gap-3">
        <span className={cx('mt-px shrink-0', style.title)}>
          <Icon name={icon ?? style.icon} />
        </span>
        <div className="min-w-0 flex-1">
          <p className={cx('text-sm font-medium', style.title)}>{title}</p>
          {children && (
            <div className="mt-1 text-sm text-pretty break-words text-gray-700">
              {children}
            </div>
          )}
          {(onRetry || action) && (
            <div className="mt-3 flex flex-wrap gap-2">
              {onRetry && (
                <Button size="sm" variant="secondary" icon="arrow-path" loading={retrying} onClick={onRetry}>
                  Try again
                </Button>
              )}
              {action}
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
