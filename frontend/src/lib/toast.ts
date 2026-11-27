/**
 * The one toast queue. Call `toast()` from anywhere (pages usually use
 * `desk.toast`, which is this function); `<Toaster />` (mounted once in App)
 * shows the queue with the HyperUI toast markup.
 */
import { useSyncExternalStore } from 'react';

/** What a toast says. `tone` picks the icon and colour; default `"success"`. */
export interface ToastInput {
  tone?: 'success' | 'error' | 'warning' | 'info';
  title: string;
  message?: string;
}

/** A queued toast. */
export interface ToastItem extends ToastInput {
  id: number;
}

let items: ToastItem[] = [];
let nextId = 0;
const listeners = new Set<() => void>();
const emit = () => listeners.forEach((listener) => listener());

/** Show a toast. It dismisses itself after 5 s (errors after 10 s). */
export function toast(input: ToastInput): void {
  // The same message twice (e.g. several calls failing at once) shows once.
  if (items.some((item) => item.title === input.title && item.message === input.message)) return;
  const id = ++nextId;
  items = [...items, { ...input, id }].slice(-4);
  emit();
  setTimeout(() => dismissToast(id), input.tone === 'error' ? 10_000 : 5_000);
}

/** Remove one toast. */
export function dismissToast(id: number): void {
  items = items.filter((item) => item.id !== id);
  emit();
}

/** Remove every toast (on sign-out, so nothing of the last session stays on screen). */
export function clearToasts(): void {
  items = [];
  emit();
}

/** Current queue, for `<Toaster />`. */
export function useToasts(): ToastItem[] {
  return useSyncExternalStore(
    (listener) => {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
    () => items,
  );
}
