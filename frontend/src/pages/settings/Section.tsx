import { useId, type ReactNode } from 'react';

/**
 * One settings block, in a single column: title, description and an optional
 * action, then the card(s) under them. Stack them inside a `divide-y` wrapper
 * (SettingsPage does this).
 *
 * ```tsx
 * <Section title="Profile" description="Your name and the email you sign in with.">
 *   <form …><Card footer={<Button type="submit">Save</Button>}>…</Card></form>
 * </Section>
 * ```
 */
export default function Section({
  title,
  description,
  action,
  children,
}: {
  title: string;
  description?: ReactNode;
  /** A button under the description, e.g. "Add staff". */
  action?: ReactNode;
  children: ReactNode;
}) {
  const id = useId();
  return (
    <section aria-labelledby={id} className="grid gap-6 py-8 first:pt-0 last:pb-0">
      <div>
        <h2 id={id} className="text-base font-semibold text-gray-900">
          {title}
        </h2>
        {description && <div className="mt-1 text-sm text-pretty text-gray-600">{description}</div>}
        {action && <div className="mt-4">{action}</div>}
      </div>
      <div className="min-w-0 space-y-4">{children}</div>
    </section>
  );
}
