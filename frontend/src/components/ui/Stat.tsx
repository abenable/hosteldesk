import type { ReactNode } from 'react';
import { Badge, type BadgeTone } from './Badge';

/** Props of {@link Stat}. */
export interface StatProps {
  label: ReactNode;
  /** Already formatted, e.g. `desk.formatMoney(stats.revenue_today)` or `"18 / 24"`. */
  value: ReactNode;
  /** Small tinted pill next to the value, e.g. `{ text: '75%', tone: 'success' }`. */
  badge?: { text: ReactNode; tone?: BadgeTone };
  /** Muted line under the value. */
  hint?: ReactNode;
}

/**
 * One cell of a {@link StatGroup}: muted label, large tabular number, optional badge.
 */
export function Stat({ label, value, badge, hint }: StatProps) {
  return (
    <div className="min-w-0 px-6 py-5">
      <p className="text-sm text-gray-500">{label}</p>
      <div className="mt-1.5 flex flex-wrap items-center gap-x-2 gap-y-1">
        <p className="text-2xl font-semibold tracking-tight break-words text-gray-900 tabular-nums">
          {value}
        </p>
        {badge && <Badge tone={badge.tone ?? 'neutral'}>{badge.text}</Badge>}
      </div>
      {hint && <p className="mt-1 text-xs text-gray-500">{hint}</p>}
    </div>
  );
}

/**
 * A strip of figures in one card, split into equal cells by hairlines
 * (HyperUI stats, tuned to the Fernhill stat strip). 2–4 cells read best;
 * money values wrap rather than truncate.
 *
 * ```tsx
 * <StatGroup>
 *   <Stat label="Rooms occupied" value="18 / 24" badge={{ text: '75%', tone: 'info' }} />
 *   <Stat label="Collected today" value={desk.formatMoney(stats.revenue_today)} />
 *   <Stat label="Outstanding" value={desk.formatMoney(stats.outstanding_total)} />
 * </StatGroup>
 * ```
 */
export function StatGroup({ children }: { children: ReactNode }) {
  return (
    <section className="grid auto-cols-fr grid-flow-col divide-x divide-gray-200 overflow-hidden rounded-xl border border-gray-200 bg-white shadow-xs">
      {children}
    </section>
  );
}
