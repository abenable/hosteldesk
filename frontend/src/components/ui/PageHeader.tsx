import type { ReactNode } from 'react';

/** Props of {@link PageHeader}. */
export interface PageHeaderProps {
  title: ReactNode;
  /** One muted line, ideally with figures: "12 rooms · 47 residents · 3 owing". */
  description?: ReactNode;
  /** Buttons on the right: outline buttons with an icon, the primary action last. */
  actions?: ReactNode;
  /** Breadcrumb back to the parent page, e.g. `{ label: 'Residents', onClick: () => desk.navigate({ page: 'residents' }) }`. */
  back?: { label: string; onClick: () => void };
}

/**
 * Top of every page (HyperUI page-headers, "With breadcrumbs and description"):
 * a large title, a muted subtitle and actions on the right. It is the page's
 * title area (the shell has no top bar), so render it in every state of the
 * page (loading, error, empty) and keep it first.
 *
 * ```tsx
 * <PageHeader title="Rooms" description={`${rooms.length} rooms · ${free} beds free`}
 *   actions={<Button variant="secondary" icon="plus" onClick={openAdd}>Add room</Button>} />
 * ```
 */
export function PageHeader({ title, description, actions, back }: PageHeaderProps) {
  return (
    <header className="flex flex-col gap-4 sm:flex-row sm:items-end sm:justify-between">
      <div className="min-w-0">
        {back && (
          <nav aria-label="Breadcrumb" className="mb-2">
            <ol className="flex items-center gap-1.5 text-sm text-gray-500">
              <li>
                <button
                  type="button"
                  onClick={back.onClick}
                  className="rounded-sm transition-colors hover:text-gray-900 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-primary-900"
                >
                  {back.label}
                </button>
              </li>
              <li aria-hidden="true" className="rtl:rotate-180">
                /
              </li>
              <li aria-current="page" className="truncate text-gray-900">
                {title}
              </li>
            </ol>
          </nav>
        )}
        <h1 className="text-3xl font-semibold tracking-tight text-gray-900">
          {title}
        </h1>
        {description && (
          <p className="mt-1 text-sm text-gray-500">{description}</p>
        )}
      </div>
      {actions && <div className="flex shrink-0 items-center gap-2">{actions}</div>}
    </header>
  );
}
