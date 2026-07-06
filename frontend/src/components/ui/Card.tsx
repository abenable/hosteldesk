import type { HTMLAttributes, ReactNode } from 'react';

type Props = HTMLAttributes<HTMLDivElement> & {
  shadow?: boolean;
  children: ReactNode;
};

export function Card({ shadow = false, className = '', children, ...rest }: Props) {
  return (
    <div
      className={`bg-canvas rounded-md ${shadow ? 'shadow-card' : ''} ${className}`}
      {...rest}
    >
      {children}
    </div>
  );
}
