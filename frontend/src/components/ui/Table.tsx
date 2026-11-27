import { useState, type Key, type ReactNode } from 'react';
import { Avatar } from './Avatar';
import { cx } from './cx';
import { Icon } from './Icon';

/** One column of a {@link Table}. */
export interface Column<Row> {
  header: ReactNode;
  cell: (row: Row) => ReactNode;
  /** `"end"` for money and counts (also makes digits line up). */
  align?: 'start' | 'end';
  /** Extra classes for this column's cells, e.g. `"whitespace-normal"` for notes. */
  className?: string;
}

/** Props of {@link Table}. */
export interface TableProps<Row> {
  columns: Column<Row>[];
  rows: Row[];
  rowKey: (row: Row) => Key;
  /** Makes rows clickable (mouse and Enter key), e.g. to open a resident. */
  onRowClick?: (row: Row) => void;
  /** Key of the selected row (accent tint), e.g. the one shown in a DetailPanel. */
  selectedKey?: Key | null;
  /** Rows per page. Adds the footer "Showing 1 to 10 of 47 residents" with prev/next. */
  pageSize?: number;
  /** What the rows are, for the footer: "residents". */
  noun?: string;
  /** Screen-reader caption. */
  caption?: string;
}

// A click or Enter on a button/link/field inside a row (e.g. its actions menu) is not a row click.
const fromControl = (target: EventTarget) =>
  target instanceof Element && target.closest('button, a, input, select, textarea, label') !== null;

const ROUND =
  'inline-flex size-9 items-center justify-center rounded-full border border-gray-200 bg-white text-gray-700 transition-colors hover:bg-gray-50 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-primary-900 disabled:cursor-not-allowed disabled:opacity-40';

/**
 * Data table (HyperUI tables, tuned to the Tabula list): small uppercase
 * header on a tinted row, divided rows, optional selection tint and paging.
 * Put it in a `<Card flush>`; render an EmptyState instead when `rows` is empty.
 *
 * ```tsx
 * <Card flush>
 *   <Table rows={residents} rowKey={(r) => r.id} pageSize={10} noun="residents"
 *     selectedKey={selected?.id} onRowClick={setSelected}
 *     columns={[
 *       { header: 'Resident', cell: (r) => <NameCell name={r.full_name} detail={r.phone ?? '—'} /> },
 *       { header: 'Balance', cell: (r) => desk.formatMoney(r.balance), align: 'end' },
 *     ]} />
 * </Card>
 * ```
 */
export function Table<Row>({
  columns,
  rows,
  rowKey,
  onRowClick,
  selectedKey,
  pageSize,
  noun = 'rows',
  caption,
}: TableProps<Row>) {
  const [wanted, setPage] = useState(0);
  const pages = pageSize ? Math.max(1, Math.ceil(rows.length / pageSize)) : 1;
  const page = Math.min(wanted, pages - 1);
  const shown = pageSize ? rows.slice(page * pageSize, (page + 1) * pageSize) : rows;
  const align = (column: Column<Row>) =>
    column.align === 'end' ? 'text-end tabular-nums' : 'text-start';

  return (
    <div>
      <div className="overflow-x-auto">
        <table className="min-w-full text-sm">
          {caption && <caption className="sr-only">{caption}</caption>}
          <thead className="bg-gray-50">
            <tr className="border-b border-gray-200">
              {columns.map((column, i) => (
                <th
                  key={i}
                  scope="col"
                  className={cx(
                    'px-5 py-2.5 text-xs font-medium tracking-wide whitespace-nowrap text-gray-500 uppercase',
                    align(column),
                  )}
                >
                  {column.header}
                </th>
              ))}
            </tr>
          </thead>
          <tbody className="divide-y divide-gray-200">
            {shown.map((row) => {
              const key = rowKey(row);
              const selected = selectedKey !== undefined && selectedKey !== null && key === selectedKey;
              return (
                <tr
                  key={key}
                  aria-selected={onRowClick ? selected : undefined}
                  onClick={onRowClick && ((e) => !fromControl(e.target) && onRowClick(row))}
                  onKeyDown={
                    onRowClick &&
                    ((e) => {
                      if (e.key === 'Enter' && !fromControl(e.target)) onRowClick(row);
                    })
                  }
                  tabIndex={onRowClick ? 0 : undefined}
                  className={cx(
                    '*:text-gray-700 *:first:font-medium *:first:text-gray-900',
                    selected && 'bg-primary-100',
                    onRowClick &&
                      !selected &&
                      'cursor-pointer transition-colors hover:bg-gray-50 focus-visible:bg-gray-50',
                    onRowClick && 'cursor-pointer focus-visible:outline-none',
                  )}
                >
                  {columns.map((column, i) => (
                    <td
                      key={i}
                      className={cx('px-5 py-3 whitespace-nowrap', align(column), column.className)}
                    >
                      {column.cell(row)}
                    </td>
                  ))}
                </tr>
              );
            })}
          </tbody>
        </table>
      </div>
      {pageSize && rows.length > 0 && (
        <div className="flex items-center justify-between gap-4 border-t border-gray-200 px-5 py-3 text-sm text-gray-500">
          <p>
            Showing {page * pageSize + 1} to {Math.min((page + 1) * pageSize, rows.length)} of{' '}
            {rows.length} {noun}
          </p>
          <div className="flex items-center gap-3">
            <button type="button" className={ROUND} disabled={page === 0} onClick={() => setPage(page - 1)}>
              <span className="sr-only">Previous page</span>
              <Icon name="chevron-left" className="size-4" />
            </button>
            <span className="font-medium text-gray-900">
              Page {page + 1} of {pages}
            </span>
            <button
              type="button"
              className={ROUND}
              disabled={page >= pages - 1}
              onClick={() => setPage(page + 1)}
            >
              <span className="sr-only">Next page</span>
              <Icon name="chevron-right" className="size-4" />
            </button>
          </div>
        </div>
      )}
    </div>
  );
}

/**
 * Two-line person cell: tinted initials avatar, name, muted second line.
 *
 * ```tsx
 * { header: 'Resident', cell: (r) => <NameCell name={r.full_name} detail={`Room ${r.room_number ?? '—'} · ${r.phone ?? ''}`} /> }
 * ```
 */
export function NameCell({ name, detail }: { name: string; detail?: ReactNode }) {
  return (
    <div className="flex items-center gap-3">
      <Avatar name={name} size="sm" />
      <div className="min-w-0">
        <p className="truncate font-medium text-gray-900">{name}</p>
        {detail && (
          <p className="truncate text-sm font-normal text-gray-500">{detail}</p>
        )}
      </div>
    </div>
  );
}
