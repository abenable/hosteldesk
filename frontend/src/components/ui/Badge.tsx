import type { HTMLAttributes, ReactNode } from 'react';

type Props = HTMLAttributes<HTMLSpanElement> & {
  children: ReactNode;
};

export function Badge({ className = '', children, ...rest }: Props) {
  return (
    <span
      className={`inline-flex items-center rounded-full bg-surface-strong px-2.5 py-0.5 text-badge text-ink ${className}`}
      {...rest}
    >
      {children}
    </span>
  );
}
