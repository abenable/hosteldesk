import { useId, useLayoutEffect, useRef, type FormEvent, type ReactNode } from 'react';
import { Alert } from './Alert';
import { Button } from './Button';
import { cx } from './cx';
import { Icon } from './Icon';

/** Props of {@link Modal}. */
export interface ModalProps {
  open: boolean;
  /** Called on Escape, the close button, or your Cancel button. Not called while `busy`. */
  onClose: () => void;
  title: ReactNode;
  description?: ReactNode;
  /** When set, the body and footer are a `<form>`: Enter in a field submits it. */
  onSubmit?: (event: FormEvent<HTMLFormElement>) => void;
  /** Buttons, right-aligned; the primary one last (`type="submit"` with `onSubmit`). */
  footer?: ReactNode;
  /** While true, Escape and the close button do nothing (a write is in flight). */
  busy?: boolean;
  /** `md` (default) for confirmations and short forms, `lg` for long forms. */
  size?: 'md' | 'lg';
  children?: ReactNode;
}

/**
 * Dialog on the native `<dialog>` element (HyperUI modals, "Invite form").
 * Opens with `showModal()` (focus is trapped, Escape closes), focuses the first
 * field (or the element marked `data-autofocus`), and returns focus to whatever
 * was focused before. The content is
 * mounted only while open, so a form inside starts fresh every time.
 *
 * ```tsx
 * <Modal open={adding} onClose={() => setAdding(false)} title="Add room" onSubmit={submit} busy={save.busy}
 *   footer={<><Button variant="secondary" onClick={() => setAdding(false)}>Cancel</Button>
 *            <Button type="submit" loading={save.busy}>Add room</Button></>}>
 *   <Field label="Room number"><Input … /></Field>
 * </Modal>
 * ```
 */
export function Modal({
  open,
  onClose,
  title,
  description,
  onSubmit,
  footer,
  busy = false,
  size = 'md',
  children,
}: ModalProps) {
  const dialog = useRef<HTMLDialogElement>(null);
  const opener = useRef<HTMLElement | null>(null);
  const id = useId();

  // Layout effect: open/close before paint, so a closing dialog never flashes empty.
  useLayoutEffect(() => {
    const el = dialog.current;
    if (!el) return;
    if (open && !el.open) {
      opener.current = document.activeElement as HTMLElement | null;
      el.showModal();
      el.querySelector<HTMLElement>(
        '[data-autofocus], input:not([type="hidden"]):not(:disabled), select:not(:disabled), textarea:not(:disabled)',
      )?.focus();
    } else if (!open && el.open) {
      el.close();
    }
  }, [open]);

  const close = () => {
    if (!busy) onClose();
  };

  const body = (
    <>
      <div className="flex items-start justify-between gap-4">
        <div>
          <h2 id={`${id}-title`} className="text-lg font-semibold text-gray-900">
            {title}
          </h2>
          {description && (
            <div id={`${id}-desc`} className="mt-1 text-sm text-pretty text-gray-600">
              {description}
            </div>
          )}
        </div>
        <button
          type="button"
          onClick={close}
          disabled={busy}
          className="-me-2 -mt-2 rounded-full p-2 text-gray-500 hover:bg-gray-100 hover:text-gray-900 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-primary-900 disabled:opacity-50"
        >
          <span className="sr-only">Close</span>
          <Icon name="x-mark" />
        </button>
      </div>
      {children && <div className="mt-5 space-y-4">{children}</div>}
      {footer && <div className="mt-6 flex justify-end gap-3">{footer}</div>}
    </>
  );

  return (
    <dialog
      ref={dialog}
      aria-labelledby={`${id}-title`}
      aria-describedby={description ? `${id}-desc` : undefined}
      onCancel={(e) => {
        e.preventDefault();
        close();
      }}
      onClose={() => {
        // The browser can force-close (e.g. Escape pressed twice). Mid-write, reopen so
        // the outcome still shows; otherwise keep React in step.
        if (open && busy) {
          dialog.current?.showModal();
          return;
        }
        if (open) onClose();
        opener.current?.focus();
      }}
      className={cx(
        'm-auto max-h-[calc(100%-2rem)] w-[calc(100%-2rem)] overflow-y-auto rounded-2xl bg-white p-6 text-start shadow-xl backdrop:bg-black/50',
        size === 'lg' ? 'max-w-2xl' : 'max-w-md',
      )}
    >
      {open && (onSubmit ? <form onSubmit={onSubmit} noValidate>{body}</form> : body)}
    </dialog>
  );
}

/** Props of {@link ConfirmDialog}. */
export interface ConfirmDialogProps {
  open: boolean;
  title: ReactNode;
  message?: ReactNode;
  /** Label of the confirming button, a verb: "Check out", "Remove". */
  confirmLabel: string;
  /** `danger` (default) for destructive actions, `primary` otherwise. */
  tone?: 'danger' | 'primary';
  busy?: boolean;
  /** Shown inside the dialog when the action failed; the dialog stays open. */
  error?: string | null;
  onConfirm: () => void;
  onCancel: () => void;
}

/**
 * Yes/no question (HyperUI modals, "Confirm"). Pages normally call
 * `desk.confirm({...})` instead of rendering this themselves.
 */
export function ConfirmDialog({
  open,
  title,
  message,
  confirmLabel,
  tone = 'danger',
  busy = false,
  error,
  onConfirm,
  onCancel,
}: ConfirmDialogProps) {
  return (
    <Modal
      open={open}
      onClose={onCancel}
      title={title}
      description={message}
      busy={busy}
      footer={
        <>
          <Button variant="secondary" onClick={onCancel} disabled={busy}>
            Cancel
          </Button>
          <Button variant={tone} loading={busy} onClick={onConfirm}>
            {confirmLabel}
          </Button>
        </>
      }
    >
      {error && <Alert title="That did not work">{error}</Alert>}
    </Modal>
  );
}
