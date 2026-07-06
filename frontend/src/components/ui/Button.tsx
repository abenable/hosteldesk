import type { ButtonHTMLAttributes, ReactNode } from 'react';

type Variant = 'primary' | 'secondary' | 'tertiary' | 'pill';
type Size = 'md' | 'sm';

type Props = ButtonHTMLAttributes<HTMLButtonElement> & {
  variant?: Variant;
  size?: Size;
  children: ReactNode;
};

const base =
  'inline-flex items-center justify-center font-sans transition-colors focus:outline-none disabled:cursor-not-allowed';

const variants: Record<Variant, string> = {
  primary:
    'bg-primary text-on-primary rounded-full h-12 px-6 text-button-md active:bg-primary-active disabled:bg-primary-disabled',
  secondary:
    'bg-canvas text-ink rounded-full h-12 px-6 text-button-md border border-ink',
  tertiary: 'bg-transparent text-ink text-button-md underline-offset-4 hover:underline',
  pill: 'bg-primary text-on-primary rounded-full px-5 py-2.5 text-button-sm active:bg-primary-active disabled:bg-primary-disabled',
};

const sizes: Record<Size, string> = {
  md: '',
  sm: 'h-10 px-4 text-button-sm',
};

export function Button({
  variant = 'primary',
  size = 'md',
  className = '',
  children,
  ...rest
}: Props) {
  return (
    <button
      type="button"
      className={`${base} ${variants[variant]} ${sizes[size]} ${className}`}
      {...rest}
    >
      {children}
    </button>
  );
}
