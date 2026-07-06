import { useEffect, useState } from 'react';
import { subscribe, removeToast, type Toast } from '../lib/toasts.ts';

const borderFor = (type: Toast['type']) =>
  type === 'success'
    ? 'border-l-4 border-l-primary'
    : type === 'error'
      ? 'border-l-4 border-l-error-text'
      : 'border-l-4 border-l-ink';

export function ToastContainer() {
  const [toasts, setToasts] = useState<Toast[]>([]);

  useEffect(() => {
    return subscribe(setToasts);
  }, []);

  if (toasts.length === 0) return null;

  return (
    <div className="fixed right-4 top-4 z-50 flex flex-col gap-2">
      {toasts.map((toast) => (
        <div
          key={toast.id}
          className={`max-w-xs rounded-md bg-canvas px-4 py-3 text-body-sm text-ink shadow-card ${borderFor(
            toast.type,
          )}`}
          role="alert"
        >
          <div className="flex items-start justify-between gap-3">
            <span>{toast.message}</span>
            <button
              type="button"
              onClick={() => removeToast(toast.id)}
              className="shrink-0 font-bold leading-none text-muted hover:text-ink"
              aria-label="Dismiss"
            >
              ×
            </button>
          </div>
        </div>
      ))}
    </div>
  );
}
