import type { ReactNode } from 'react';
import { cx } from './cx';

/** Props of {@link Card}. */
export interface CardProps {
  title?: ReactNode;
  description?: ReactNode;
  /** Buttons at the top right of the card. */
  actions?: ReactNode;
  /** Bottom bar (gray), right-aligned: usually the Save button of a settings form. */
  footer?: ReactNode;
  /** No body padding: for a `Table` or a divided list that runs edge to edge. */
  flush?: boolean;
  className?: string;
  children?: ReactNode;
}

/**
 * White panel with a hairline border and a very soft shadow (HyperUI
 * form-layouts "Stacked cards"). Dropdown menus inside are not clipped (they are fixed).
 *
 * ```tsx
 * <Card title="Business" description="Shown on receipts." footer={<Button type="submit">Save</Button>}>…</Card>
 * <Card title="Residents" flush><Table … /></Card>
 * ```
 */
export function Card({ title, description, actions, footer, flush, className, children }: CardProps) {
  const header = title || description || actions;
  return (
    <section
      className={cx(
        'flex flex-col overflow-hidden rounded-xl border border-gray-200 bg-white shadow-xs',
        className,
      )}
    >
      {header && (
        <div className={cx('flex items-start justify-between gap-4 px-6 pt-6', flush && 'pb-4')}>
          <div className="min-w-0">
            {title && (
              <h2 className="text-base font-semibold text-gray-900">{title}</h2>
            )}
            {description && (
              <p className="mt-1 text-sm text-gray-600">{description}</p>
            )}
          </div>
          {actions && <div className="flex shrink-0 items-center gap-2">{actions}</div>}
        </div>
      )}
      <div className={cx('flex-1', !flush && (header ? 'px-6 pt-6 pb-6' : 'p-6'))}>{children}</div>
      {footer && (
        <div className="flex justify-end gap-3 border-t border-gray-200 bg-gray-50 px-6 py-3">
          {footer}
        </div>
      )}
    </section>
  );
}
