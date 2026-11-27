import { useEffect, useRef, type ReactNode } from 'react';
import { BrandMark, cx } from '../../components/ui';

/**
 * Centred column for the gate screens: logo, title and description above the
 * content (usually one Card), optional small print below. Like a dialog, it
 * puts the cursor in the first field when it opens.
 */
export default function AuthLayout({
  title,
  description,
  footer,
  wide = false,
  children,
}: {
  title: ReactNode;
  description?: ReactNode;
  footer?: ReactNode;
  /** `max-w-xl` instead of `max-w-md`, for longer forms. */
  wide?: boolean;
  children: ReactNode;
}) {
  const root = useRef<HTMLElement>(null);
  useEffect(() => {
    root.current?.querySelector<HTMLElement>('input:not([type="hidden"]):not(:disabled)')?.focus();
  }, []);

  return (
    <main ref={root} className="flex min-h-screen flex-col items-center justify-center px-4 py-12">
      <div className={cx('w-full', wide ? 'max-w-xl' : 'max-w-md')}>
        <div className="mb-8 text-center">
          <BrandMark size="lg" />
          <p className="mt-3 text-sm font-semibold text-gray-900">HostelDesk</p>
          <h1 className="mt-6 text-3xl font-semibold tracking-tight text-gray-900">{title}</h1>
          {description && (
            <p className="mt-2 text-sm text-pretty text-gray-500">{description}</p>
          )}
        </div>
        <div className="space-y-6">{children}</div>
        {footer && (
          <p className="mt-8 text-center text-xs text-gray-500">{footer}</p>
        )}
      </div>
    </main>
  );
}
