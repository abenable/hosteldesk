import type { InputHTMLAttributes, ReactNode } from 'react';
import { useState } from 'react';

type Props = InputHTMLAttributes<HTMLInputElement> & {
  label?: string;
  error?: string;
  hint?: ReactNode;
};

export function Input({ label, error, hint, id, className = '', type, ...rest }: Props) {
  const inputId = id ?? rest.name;
  const [visible, setVisible] = useState(false);
  const isPassword = type === 'password';
  const effectiveType = isPassword ? (visible ? 'text' : 'password') : type;

  return (
    <div className="flex flex-col gap-1">
      {label && (
        <label htmlFor={inputId} className="text-caption text-muted">
          {label}
        </label>
      )}
      <div className="relative">
        <input
          id={inputId}
          type={effectiveType}
          className={`h-9 w-full rounded-full border bg-canvas px-4 text-body-sm text-ink placeholder:text-muted-soft focus:border-ink focus:outline-none ${
            error ? 'border-error-text' : 'border-hairline'
          } focus:border-2 ${isPassword ? 'pr-10' : ''} ${className}`}
          aria-invalid={error ? true : undefined}
          aria-describedby={error ? `${inputId}-error` : undefined}
          {...rest}
        />
        {isPassword && (
          <button
            type="button"
            onClick={() => setVisible((v) => !v)}
            className="absolute right-2 top-1/2 -translate-y-1/2 rounded-full px-3 py-1 text-caption-sm text-muted hover:bg-surface-soft hover:text-ink"
            aria-label={visible ? 'Hide password' : 'Show password'}
          >
            {visible ? 'Hide' : 'Show'}
          </button>
        )}
      </div>
      {error && (
        <p id={`${inputId}-error`} className="text-caption-sm text-error-text">
          {error}
        </p>
      )}
      {!error && hint && <p className="text-caption-sm text-muted">{hint}</p>}
    </div>
  );
}
