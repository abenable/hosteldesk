import type { ReactNode } from 'react';
import { Icon, type IconName } from './Icon';

/** Props of {@link EmptyState}. */
export interface EmptyStateProps {
  icon?: IconName;
  title: ReactNode;
  description?: ReactNode;
  /** The next step, usually one primary `Button`. */
  action?: ReactNode;
}

/**
 * Nothing here yet (HyperUI empty-states, "Create first item" with the icon
 * bubble of "No results with filters"). Use it inside a Card or on its own.
 *
 * ```tsx
 * <EmptyState icon="home" title="No rooms yet" description="Add your rooms to start checking residents in."
 *   action={<Button icon="plus" onClick={openAdd}>Add room</Button>} />
 * ```
 */
export function EmptyState({ icon = 'inbox', title, description, action }: EmptyStateProps) {
  return (
    <div className="mx-auto max-w-md px-6 py-12 text-center">
      <span className="mx-auto flex size-12 items-center justify-center rounded-xl bg-primary-100 text-primary-700 ring-1 ring-primary-900/5">
        <Icon name={icon} className="size-6" />
      </span>
      <h2 className="mt-4 text-lg font-semibold text-gray-900">{title}</h2>
      {description && (
        <p className="mt-2 text-sm text-pretty text-gray-600">{description}</p>
      )}
      {action && (
        <div className="mt-6 flex flex-wrap items-center justify-center gap-x-4 gap-y-3">
          {action}
        </div>
      )}
    </div>
  );
}
