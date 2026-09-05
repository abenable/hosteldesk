import type { ReactNode } from 'react';
import * as Dialog from '@radix-ui/react-dialog';
import { CloseIcon } from './icons.tsx';

type Props = {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  children: ReactNode;
};

export function DetailDrawer({ open, onOpenChange, children }: Props) {
  return (
    <Dialog.Root open={open} onOpenChange={onOpenChange}>
      <Dialog.Portal>
        <Dialog.Overlay className="fixed inset-0 z-40 bg-scrim/40 backdrop-blur-sm data-[state=open]:animate-in data-[state=open]:fade-in" />
        <Dialog.Content className="fixed right-0 top-0 z-50 flex h-full w-full max-w-[420px] flex-col overflow-y-auto border-l border-hairline bg-canvas p-6 shadow-card focus:outline-none">
          <Dialog.Close
            className="absolute right-4 top-4 rounded-full p-1 text-muted hover:bg-surface-soft hover:text-ink"
            aria-label="Close"
          >
            <CloseIcon width={18} height={18} />
          </Dialog.Close>
          {children}
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}
