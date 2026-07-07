import type { ReactNode } from 'react';
import * as Dialog from '@radix-ui/react-dialog';
import { CloseIcon } from './icons.tsx';

type Props = {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  title: string;
  description?: string;
  children: ReactNode;
  footer?: ReactNode;
};

export function Modal({ open, onOpenChange, title, description, children, footer }: Props) {
  return (
    <Dialog.Root open={open} onOpenChange={onOpenChange}>
      <Dialog.Portal>
        <Dialog.Overlay className="fixed inset-0 z-40 bg-scrim/40 backdrop-blur-sm data-[state=open]:animate-in data-[state=open]:fade-in" />
        <Dialog.Content className="fixed left-1/2 top-1/2 z-50 w-full max-w-lg -translate-x-1/2 -translate-y-1/2 rounded-lg bg-canvas p-6 shadow-card focus:outline-none">
          <div className="mb-4 flex items-start justify-between gap-4">
            <div>
              <Dialog.Title className="text-display-sm text-ink">{title}</Dialog.Title>
              {description && (
                <Dialog.Description className="mt-1 text-body-sm text-muted">
                  {description}
                </Dialog.Description>
              )}
            </div>
            <Dialog.Close
              className="rounded-full p-1 text-muted hover:bg-surface-soft hover:text-ink"
              aria-label="Close"
            >
              <CloseIcon width={18} height={18} />
            </Dialog.Close>
          </div>
          <div className="flex flex-col gap-4">{children}</div>
          {footer && (
            <div className="mt-6 flex items-center justify-end gap-3">{footer}</div>
          )}
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}
