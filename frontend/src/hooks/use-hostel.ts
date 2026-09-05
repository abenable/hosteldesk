import { createContext, useContext } from 'react';
import { type Hostel } from '@/lib/tauri.ts';

export type HostelContextValue = {
  hostel: Hostel | null;
  hostelId: number;
  currency: string;
  refresh: () => void;
  loading: boolean;
};

export const HostelContext = createContext<HostelContextValue | null>(null);

export function useHostel(): HostelContextValue {
  const ctx = useContext(HostelContext);
  if (!ctx) throw new Error('useHostel must be used within HostelProvider');
  return ctx;
}
