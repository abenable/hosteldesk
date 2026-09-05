import { useCallback, useEffect, useMemo, useState, type ReactNode } from 'react';
import { listHostels } from './tauri.ts';
import { HostelContext, type HostelContextValue } from '@/hooks/use-hostel.ts';

export type { HostelContextValue } from '@/hooks/use-hostel.ts';

export function HostelProvider({ children }: { children: ReactNode }) {
  const [hostel, setHostel] = useState<HostelContextValue['hostel']>(null);
  const [loading, setLoading] = useState(true);

  const load = useCallback(() => {
    setLoading(true);
    listHostels()
      .then((h) => setHostel(h[0] ?? null))
      .catch(() => setHostel(null))
      .finally(() => setLoading(false));
  }, []);

  useEffect(() => {
    load();
  }, [load]);

  const value: HostelContextValue = useMemo(
    () => ({
      hostel,
      hostelId: hostel?.id ?? 0,
      currency: hostel?.currency ?? 'UGX',
      refresh: load,
      loading,
    }),
    [hostel, load, loading],
  );

  return <HostelContext.Provider value={value}>{children}</HostelContext.Provider>;
}
