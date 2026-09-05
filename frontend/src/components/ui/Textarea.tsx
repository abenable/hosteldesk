import type { TextareaHTMLAttributes } from 'react';

type Props = TextareaHTMLAttributes<HTMLTextAreaElement> & {
  label?: string;
  error?: string;
};

export function Textarea({ label, error, id, className = '', ...rest }: Props) {
  const textareaId = id ?? rest.name;
  return (
    <div className="flex flex-col gap-1">
      {label && (
        <label htmlFor={textareaId} className="text-caption text-muted">
          {label}
        </label>
      )}
      <textarea
        id={textareaId}
        className={`w-full rounded-lg border bg-canvas px-4 py-2 text-body-sm text-ink placeholder:text-muted-soft focus:border-ink focus:outline-none ${
          error ? 'border-error-text' : 'border-hairline'
        } focus:border-2 ${className}`}
        aria-invalid={error ? true : undefined}
        {...rest}
      />
      {error && (
        <p className="text-caption-sm text-error-text">{error}</p>
      )}
    </div>
  );
}
