import { useContext, type ReactNode } from 'react';
import { createPortal } from 'react-dom';
import { DetailSlot } from './detailSlot';
import { Icon } from './Icon';

/** Props of {@link DetailPanel}. */
export interface DetailPanelProps {
  title: ReactNode;
  /** Muted line under the title, e.g. "Room 12 · since 2 Sep 2026". */
  subtitle?: ReactNode;
  onClose: () => void;
  /**
   * Sticky bottom bar. Convention: a secondary outline button, then the
   * primary action stretched: `<Button className="flex-1">Take payment</Button>`.
   */
  footer?: ReactNode;
  /** Sections: `DetailSection`, `MiniStats`, `Alert` (tinted callouts), lists. */
  children: ReactNode;
}

/**
 * Right-hand detail panel docked beside the page (like a mail reader): header
 * with title, subtitle and close button, scrolling body, sticky footer. Render
 * it conditionally from the page, `{selected && <DetailPanel … />}`; the shell
 * gives it a column on the right of the content (it portals there), and the
 * column disappears when no panel is open. Escape is not bound: use the close
 * button or select another row.
 *
 * ```tsx
 * <DetailPanel title={r.full_name} subtitle={`Room ${r.room_number}`} onClose={() => setSelected(null)}
 *   footer={<><Button variant="secondary" icon="pencil-square">Edit</Button><Button className="flex-1">Take payment</Button></>}>
 *   <MiniStats items={[{ label: 'Balance', value: desk.formatMoney(r.balance) }]} />
 *   <DetailSection label="Contact">…</DetailSection>
 * </DetailPanel>
 * ```
 */
export function DetailPanel({ title, subtitle, onClose, footer, children }: DetailPanelProps) {
  const slot = useContext(DetailSlot);
  const panel = (
    <section aria-label="Details" className="flex h-full min-h-0 flex-col">
      <header className="flex items-start justify-between gap-4 border-b border-gray-200 px-6 py-5">
        <div className="min-w-0">
          <h2 className="truncate text-xl font-semibold text-gray-900">{title}</h2>
          {subtitle && (
            <p className="mt-0.5 text-sm text-gray-500">{subtitle}</p>
          )}
        </div>
        <button
          type="button"
          onClick={onClose}
          className="-me-2 rounded-full p-2 text-gray-500 hover:bg-gray-100 hover:text-gray-900 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-primary-900"
        >
          <span className="sr-only">Close</span>
          <Icon name="x-mark" />
        </button>
      </header>
      <div className="min-h-0 flex-1 space-y-6 overflow-y-auto px-6 py-5">{children}</div>
      {footer && (
        <footer className="flex items-center gap-3 border-t border-gray-200 px-6 py-4">
          {footer}
        </footer>
      )}
    </section>
  );
  return slot ? createPortal(panel, slot) : panel;
}

/**
 * A block of the detail panel with a small uppercase label.
 *
 * ```tsx
 * <DetailSection label="Recent payments"><ul className="divide-y …">…</ul></DetailSection>
 * ```
 */
export function DetailSection({ label, children }: { label: ReactNode; children: ReactNode }) {
  return (
    <div>
      <h3 className="text-xs font-medium tracking-wide text-gray-500 uppercase">
        {label}
      </h3>
      <div className="mt-2 text-sm text-gray-900">{children}</div>
    </div>
  );
}

/**
 * Small tinted tiles of figures, e.g. Balance / Paid / Rate in the detail panel.
 *
 * ```tsx
 * <MiniStats items={[{ label: 'Balance', value: desk.formatMoney(detail.balance) }, { label: 'Stays', value: 3 }]} />
 * ```
 */
export function MiniStats({ items }: { items: { label: ReactNode; value: ReactNode }[] }) {
  return (
    <dl className="grid auto-cols-fr grid-flow-col gap-2">
      {items.map((item, i) => (
        <div key={i} className="min-w-0 rounded-xl bg-gray-100 px-3 py-2.5">
          <dt className="truncate text-xs font-medium tracking-wide text-gray-500 uppercase">
            {item.label}
          </dt>
          <dd className="mt-1 text-base font-semibold break-words text-gray-900 tabular-nums">
            {item.value}
          </dd>
        </div>
      ))}
    </dl>
  );
}
