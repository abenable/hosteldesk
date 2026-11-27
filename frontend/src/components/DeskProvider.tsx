import { useCallback, useEffect, useMemo, useRef, useState, type ReactNode } from 'react';
import * as api from '../lib/api';
import { DeskContext, type ConfirmOptions, type DeskValue, type Route, type SyncState } from '../lib/desk';
import { formatDate, formatDateTime } from '../lib/format';
import { formatMoney } from '../lib/money';
import { clearToasts, toast } from '../lib/toast';
import type { AccountLicense, AppPreferences, BootState, HostelProfile, SyncEvent, User } from '../lib/types';
import BootFailed from '../pages/auth/BootFailed';
import { ConfirmDialog, Spinner } from './ui';

interface Base {
  profile: HostelProfile;
  prefs: AppPreferences;
  license: AccountLicense | null;
  /** The signed-in user as stored now, so a name or role change shows without signing in again. */
  user: User;
}

interface PendingConfirm {
  options: ConfirmOptions;
  resolve: (confirmed: boolean) => void;
  busy: boolean;
  error: string | null;
}

/**
 * Provides {@link DeskContext} for one signed-in session. App renders it with
 * `key={user.id}` and only on the `ready` gate, so all of its state (route,
 * data, sync status, open dialogs) is dropped when the session ends.
 *
 * It loads the hostel profile, preferences and license before showing the
 * shell, listens to the background `sync` event (toast + `dataVersion` bump on
 * changes, top-bar indicator on errors, re-boot when the license lapses), and
 * hosts the one ConfirmDialog behind `desk.confirm`.
 */
export default function DeskProvider({
  boot,
  user,
  reboot,
  children,
}: {
  boot: BootState;
  user: User;
  /** Re-run `boot_state` without the full-screen spinner (App's gate). */
  reboot: () => Promise<void>;
  children: ReactNode;
}) {
  const hostelId = user.hostel_id;
  const [route, setRoute] = useState<Route>({ page: 'dashboard' });
  const [dataVersion, setDataVersion] = useState(0);
  const [base, setBase] = useState<Base | null>(null);
  const [baseError, setBaseError] = useState<string | null>(null);
  const [retry, setRetry] = useState(0);
  const [sync, setSync] = useState<SyncState>({
    enrolled: boot.enrolled,
    lastAt: null,
    error: null,
    busy: false,
  });
  const [pending, setPending] = useState<PendingConfirm | null>(null);

  const reload = useCallback(() => setDataVersion((v) => v + 1), []);

  useEffect(() => {
    let live = true;
    Promise.all([
      api.hostelProfile(hostelId),
      api.appPreferences({ hostel_id: hostelId }),
      api.licenseStatus(),
      api.session(),
    ]).then(
      ([profile, prefs, license, current]) => {
        if (!live) return;
        if (profile) {
          // A null session means it expired; the next guarded call reboots to sign-in.
          setBase({ profile, prefs, license, user: current ?? user });
          setBaseError(null);
        } else {
          setBaseError('This hostel is no longer on this computer.');
        }
      },
      (error: unknown) => live && setBaseError(api.errorMessage(error)),
    );
    return () => {
      live = false;
    };
  }, [hostelId, dataVersion, retry, user]);

  const onSync = useCallback(
    (event: SyncEvent) => {
      setSync((s) => ({
        ...s,
        enrolled: true,
        error: event.error,
        lastAt: event.error ? s.lastAt : new Date(),
      }));
      if (event.summary) toast({ tone: 'warning', title: event.summary });
      if (event.changed) reload();
      // The license lapsed (or the offline grace ran out): back to the gate, which locks.
      if (!event.licensed) void reboot();
    },
    [reload, reboot],
  );

  useEffect(() => api.onSync(onSync), [onSync]);

  const syncing = useRef(false);
  const syncNow = useCallback(() => {
    if (syncing.current) return;
    syncing.current = true;
    setSync((s) => ({ ...s, busy: true }));
    api
      .syncNow()
      .then(onSync, (error: unknown) =>
        setSync((s) => ({ ...s, error: api.errorMessage(error) })),
      )
      .finally(() => {
        syncing.current = false;
        setSync((s) => ({ ...s, busy: false }));
      });
  }, [onSync]);

  // The background loop's first pass waits 30 s (its event would beat our
  // listener), so the startup pass runs from here.
  const started = useRef(false);
  useEffect(() => {
    if (started.current || !boot.enrolled) return;
    started.current = true;
    syncNow();
  }, [boot.enrolled, syncNow]);

  const signOut = useCallback(() => {
    api.logout().then(
      () => {
        clearToasts();
        toast({ tone: 'success', title: 'Signed out.' });
        void reboot();
      },
      (error: unknown) =>
        toast({ tone: 'error', title: 'Could not sign out', message: api.errorMessage(error) }),
    );
  }, [reboot]);

  const confirm = useCallback(
    (options: ConfirmOptions) =>
      new Promise<boolean>((resolve) =>
        setPending({ options, resolve, busy: false, error: null }),
      ),
    [],
  );

  const onConfirm = async () => {
    if (!pending || pending.busy) return;
    const { options, resolve } = pending;
    if (!options.action) {
      setPending(null);
      resolve(true);
      return;
    }
    setPending({ ...pending, busy: true, error: null });
    try {
      await options.action();
      setPending(null);
      resolve(true);
    } catch (error) {
      setPending((p) => p && { ...p, busy: false, error: api.errorMessage(error) });
    }
  };

  const onCancel = () => {
    if (!pending || pending.busy) return;
    setPending(null);
    pending.resolve(false);
  };

  const value = useMemo<DeskValue | null>(() => {
    if (!base) return null;
    const currency = base.profile.currency;
    const dateFormat = base.prefs.date_format;
    return {
      user: base.user,
      hostelId,
      isAdmin: base.user.role === 'admin',
      profile: base.profile,
      prefs: base.prefs,
      currency,
      license: base.license,
      appVersion: boot.app_version,
      route,
      navigate: setRoute,
      toast,
      confirm,
      dataVersion,
      reload,
      formatMoney: (amount) => formatMoney(amount, currency),
      formatDate: (date) => formatDate(date, dateFormat),
      formatDateTime: (date) => formatDateTime(date, dateFormat),
      sync,
      syncNow,
      signOut,
    };
  }, [base, hostelId, boot.app_version, route, confirm, dataVersion, reload, sync, syncNow, signOut]);

  if (!value) {
    return baseError ? (
      <BootFailed message={baseError} onRetry={() => setRetry((r) => r + 1)} onSignOut={signOut} />
    ) : (
      <div className="grid min-h-screen place-items-center">
        <Spinner size="lg" label="Opening your hostel…" />
      </div>
    );
  }

  return (
    <DeskContext value={value}>
      {children}
      <ConfirmDialog
        open={pending !== null}
        title={pending?.options.title ?? ''}
        message={pending?.options.message}
        confirmLabel={pending?.options.confirmLabel ?? 'OK'}
        tone={pending?.options.tone}
        busy={pending?.busy}
        error={pending?.error}
        onConfirm={() => void onConfirm()}
        onCancel={onCancel}
      />
    </DeskContext>
  );
}
