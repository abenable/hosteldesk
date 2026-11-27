import { dismissToast, useToasts, type ToastInput } from '../../lib/toast';
import { cx } from './cx';
import { Icon, type IconName } from './Icon';

const TONES: Record<NonNullable<ToastInput['tone']>, { box: string; icon: IconName }> = {
  success: {
    box: 'bg-green-50 text-green-600 ring-green-600/20',
    icon: 'check',
  },
  error: {
    box: 'bg-red-50 text-red-600 ring-red-600/20',
    icon: 'exclamation-circle',
  },
  warning: {
    box: 'bg-amber-50 text-amber-600 ring-amber-600/20',
    icon: 'exclamation-triangle',
  },
  info: {
    box: 'bg-primary-100 text-primary-700 ring-primary-900/10',
    icon: 'arrow-path',
  },
};

/**
 * The toast stack (HyperUI toasts, "Success" / "Error with retry"), bottom-right.
 * Mounted once in App; show toasts with `desk.toast(...)` or `toast(...)` from lib/toast.
 */
export function Toaster() {
  const items = useToasts();
  return (
    <div
      aria-live="polite"
      className="pointer-events-none fixed end-4 bottom-4 z-50 flex w-full max-w-sm flex-col gap-3"
    >
      {items.map((item) => {
        const tone = TONES[item.tone ?? 'success'];
        return (
          <div
            key={item.id}
            role={item.tone === 'error' ? 'alert' : 'status'}
            className="pointer-events-auto overflow-hidden rounded-xl bg-white p-4 shadow-xl ring-1 ring-gray-950/10"
          >
            <div className="flex items-start gap-3">
              <span
                className={cx(
                  'flex size-9 shrink-0 items-center justify-center rounded-lg ring-1 ring-inset',
                  tone.box,
                )}
              >
                <Icon name={tone.icon} />
              </span>
              <div className="min-w-0 flex-1">
                <p className="text-sm font-semibold text-gray-900">{item.title}</p>
                {item.message && (
                  <p className="mt-1 text-sm break-words text-gray-600">
                    {item.message}
                  </p>
                )}
              </div>
              <button
                type="button"
                onClick={() => dismissToast(item.id)}
                className="-m-1 shrink-0 rounded-full p-1 text-gray-400 hover:bg-gray-100 hover:text-gray-700 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-gray-900"
              >
                <span className="sr-only">Dismiss</span>
                <Icon name="x-mark" />
              </button>
            </div>
          </div>
        );
      })}
    </div>
  );
}
