/**
 * The two hooks pages use to talk to Rust (through src/lib/api.ts):
 *
 * - `useLoad` for reads: `const rooms = useLoad(() => api.rooms(hostelId), [hostelId])`.
 * - `useAction` for writes: one per button/form, so a write can never be sent twice.
 *
 * ## Forms and background reloads (read this before writing a form)
 *
 * `dataVersion` bumps (a sync pulled changes, another page wrote something)
 * make every `useLoad` fetch again. That must never overwrite what the user is
 * typing, so:
 *
 * 1. Keep form values in the form's own `useState`, seeded ONCE from loaded data.
 * 2. Render the form as a child that receives the loaded record as `initial`,
 *    keyed by the record id: `<ResidentForm key={detail.resident.id} initial={detail.resident} />`
 *    and inside `const [values, setValues] = useState(() => toForm(initial))`.
 * 3. Never copy `useLoad` data into form state from a `useEffect`.
 *
 * This works because `useLoad` keeps the previous `data` on screen during a
 * background reload and only sets `loading` for the first fetch of new `deps`,
 * so a `if (loading) return <Skeleton />` never unmounts a form mid-edit.
 * After the user saves, call `desk.reload()`; the form keeps its own values.
 */
import { useCallback, useContext, useEffect, useEffectEvent, useRef, useState } from 'react';
import { errorMessage } from './api';
import { DeskContext } from './desk';

/** What `useLoad` returns. */
export interface Load<T> {
  /** Last successful result; kept while a background reload runs. `undefined` until the first one. */
  data: T | undefined;
  /** Message of the last failure, or null. Cleared by the next success. */
  error: string | null;
  /** True only while there is no data for the current `deps` yet (first load, or after deps change). */
  loading: boolean;
  /** Fetch again now (e.g. a Retry button). */
  reload: () => void;
}

/**
 * Run `fn` now, again whenever `deps` change, and again on every `dataVersion`
 * bump. Responses that arrive out of order are dropped. `deps` are compared by
 * value (JSON), so pass ids, strings and filter objects, not functions.
 *
 * ```tsx
 * const { data, error, loading, reload } = useLoad(() => api.listResidents({ hostel_id: hostelId }), [hostelId]);
 * if (loading) return <Skeleton rows={5} />;
 * if (error && !data) return <Alert tone="danger" title="Could not load residents" onRetry={reload}>{error}</Alert>;
 * ```
 */
export function useLoad<T>(fn: () => Promise<T>, deps: readonly unknown[]): Load<T> {
  const dataVersion = useContext(DeskContext)?.dataVersion ?? 0;
  const [state, setState] = useState<{ key: string; data: T | undefined; error: string | null }>({
    key: '',
    data: undefined,
    error: null,
  });
  const [tick, setTick] = useState(0);
  const latest = useRef(0);
  const load = useEffectEvent(fn);
  const key = JSON.stringify(deps);

  useEffect(() => {
    const id = ++latest.current;
    load().then(
      (data) => {
        if (id === latest.current) setState({ key, data, error: null });
      },
      (error: unknown) => {
        if (id === latest.current)
          setState((s) => ({ key, data: s.key === key ? s.data : undefined, error: errorMessage(error) }));
      },
    );
  }, [key, dataVersion, tick]);

  const fresh = state.key === key;
  return {
    data: fresh ? state.data : undefined,
    error: fresh ? state.error : null,
    loading: !fresh || (state.data === undefined && state.error === null),
    reload: useCallback(() => setTick((t) => t + 1), []),
  };
}

/** What `useAction` returns. */
export interface Action {
  /** True while the action runs. Pass it to the submit Button's `loading`. */
  busy: boolean;
  /** Message of the last failure, or null. Show it next to the form (Alert or Field error). */
  error: string | null;
  /** Clear or set the error yourself (e.g. a client-side validation message). */
  setError: (error: string | null) => void;
  /**
   * Run `fn` unless this action is already running (a second click or Enter is
   * ignored). Resolves `true` when `fn` finished without throwing, `false` when
   * it threw (the message is in `error`) or was ignored. Do the success work
   * (toast, close, `desk.reload()`) inside `fn` after the await, or on `true`.
   */
  run: (fn: () => Promise<unknown>) => Promise<boolean>;
}

/**
 * One write (a form submit, a button). Prevents double submits even before
 * React re-renders the disabled button.
 *
 * ```tsx
 * const save = useAction();
 * const submit = (e: FormEvent) => {
 *   e.preventDefault();
 *   if (!valid()) return;
 *   void save.run(async () => {
 *     await api.addRooms(request);
 *     desk.toast({ tone: 'success', title: 'Room added.' });
 *     desk.reload();
 *     onClose();
 *   });
 * };
 * <form onSubmit={submit}> … <Button type="submit" loading={save.busy}>Add room</Button>
 * ```
 */
export function useAction(): Action {
  const running = useRef(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const run = useCallback(async (fn: () => Promise<unknown>) => {
    if (running.current) return false;
    running.current = true;
    setBusy(true);
    setError(null);
    try {
      await fn();
      return true;
    } catch (e) {
      setError(errorMessage(e));
      return false;
    } finally {
      running.current = false;
      setBusy(false);
    }
  }, []);

  return { busy, error, setError, run };
}
