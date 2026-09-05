import type { InputHTMLAttributes } from 'react';
import { SearchIcon } from './icons.tsx';

type Props = InputHTMLAttributes<HTMLInputElement>;

export function SearchInput({ className = '', ...rest }: Props) {
  return (
    <div className={`relative ${className}`}>
      <span className="pointer-events-none absolute left-3 top-1/2 -translate-y-1/2 text-muted">
        <SearchIcon width={16} height={16} />
      </span>
      <input
        type="search"
        className="h-9 w-full rounded-full border border-hairline bg-canvas pl-9 pr-4 text-body-sm text-ink placeholder:text-muted-soft focus:border-ink focus:outline-none focus:border-2"
        {...rest}
      />
    </div>
  );
}
