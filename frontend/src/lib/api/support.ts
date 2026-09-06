import { addToast } from '../toasts.ts';

// Invoke convention: top-level argument keys are snake_case. Commands that take
// bare parameters are annotated with rename_all = "snake_case" on the Rust
// side; commands that take a single `request` struct pass it as `{ request }`.

export function toMessage(error: unknown): string {
  if (error instanceof Error) return error.message;
  return typeof error === 'string' ? error : String(error);
}

export function reportError(error: unknown) {
  addToast(toMessage(error), 'error');
}
