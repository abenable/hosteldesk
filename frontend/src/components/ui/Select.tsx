import type { SelectHTMLAttributes, ReactNode } from 'react';

type Props = SelectHTMLAttributes<HTMLSelectElement> & {
  label?: string;
  error?: string;
  /** When true, each option's text is Capitalized for display. */
  capitalize?: boolean;
};

export function Select({
  label,
  error,
  capitalize,
  id,
  className = '',
  children,
  ...rest
}: Props) {
  const selectId = id ?? rest.name;
  return (
    <div className="flex flex-col gap-1">
      {label && (
        <label htmlFor={selectId} className="text-caption text-muted">
          {label}
        </label>
      )}
      <div className="relative">
        <select
          id={selectId}
          className={`h-14 w-full appearance-none rounded-full border bg-canvas px-5 pr-10 text-body-md text-ink focus:border-ink focus:outline-none ${
            error ? 'border-error-text' : 'border-hairline'
          } focus:border-2 ${capitalize ? 'capitalize' : ''} ${className}`}
          aria-invalid={error ? true : undefined}
          {...rest}
        >
          {children}
        </select>
        <span className="pointer-events-none absolute right-4 top-1/2 -translate-y-1/2 text-muted">
          ▾
        </span>
      </div>
      {error && <p className="text-caption-sm text-error-text">{error}</p>}
    </div>
  );
}

type OptionProps = {
  value: string;
  children: ReactNode;
};

export function SelectOption({ value, children }: OptionProps) {
  return (
    <option value={value} className="capitalize">
      {children}
    </option>
  );
}
