import { cx } from './cx';
import { Icon } from './Icon';

/**
 * Where a setup wizard is: done steps get a check, the current one is
 * filled black, the rest wait in grey.
 *
 * @example
 * <Steps labels={['Hostel', 'Administrator', 'Room classes', 'Rooms']} current={2} />
 */
export function Steps({ labels, current }: { labels: string[]; current: number }) {
  return (
    <ol className="flex flex-wrap items-center gap-x-3 gap-y-2 text-sm">
      {labels.map((label, i) => (
        <li key={label} className="flex items-center gap-2" aria-current={i === current ? 'step' : undefined}>
          <span
            aria-hidden="true"
            className={cx(
              'inline-flex size-6 shrink-0 items-center justify-center rounded-full text-xs font-semibold',
              i <= current ? 'bg-gray-950 text-white' : 'bg-gray-100 text-gray-500',
            )}
          >
            {i < current ? <Icon name="check" className="size-3.5" /> : i + 1}
          </span>
          <span className={i === current ? 'font-semibold text-gray-900' : 'text-gray-500'}>
            {label}
            {i < current && <span className="sr-only"> (done)</span>}
          </span>
          {i < labels.length - 1 && <span aria-hidden="true" className="h-px w-5 bg-gray-300" />}
        </li>
      ))}
    </ol>
  );
}
