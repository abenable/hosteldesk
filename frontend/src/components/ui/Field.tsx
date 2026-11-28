import { createContext, useContext, useId, type ComponentProps, type ReactNode } from 'react';
import { cx } from './cx';
import { Icon } from './Icon';

const FieldContext = createContext<{ id: string; describedBy?: string; invalid: boolean } | null>(
  null,
);

/** Props of {@link Field}. */
export interface FieldProps {
  label: ReactNode;
  /** Help under the control; replaced by `error` when there is one. */
  hint?: ReactNode;
  /** Field-level validation message. Marks the control invalid. */
  error?: string | null;
  className?: string;
  /** One control: `Input`, `Select` or `Textarea` (they pick up the id and aria wiring). */
  children: ReactNode;
}

/**
 * Label + control + hint/error (HyperUI form-layouts / inputs). The control
 * inside gets its `id`, `aria-invalid` and `aria-describedby` automatically.
 *
 * ```tsx
 * <Field label="Room number" error={errors.room_number}>
 *   <Input value={values.room_number} onChange={(e) => set('room_number', e.target.value)} />
 * </Field>
 * ```
 */
export function Field({ label, hint, error, className, children }: FieldProps) {
  const id = useId();
  const noteId = `${id}-note`;
  const note = error || hint;
  return (
    <div className={className}>
      <label htmlFor={id} className="block text-sm font-medium text-gray-900">
        {label}
      </label>
      <FieldContext value={{ id, describedBy: note ? noteId : undefined, invalid: !!error }}>
        <div className="mt-1.5">{children}</div>
      </FieldContext>
      {note && (
        <p
          id={noteId}
          className={cx(
            'mt-1.5 text-xs',
            error ? 'text-red-600' : 'text-gray-600',
          )}
        >
          {note}
        </p>
      )}
    </div>
  );
}

const CONTROL =
  'block w-full border-gray-300 text-sm text-gray-900 shadow-xs placeholder:text-gray-400 focus:border-primary-900 focus:ring-1 focus:ring-primary-900 disabled:cursor-not-allowed disabled:bg-gray-50 disabled:text-gray-500 aria-invalid:border-red-600 aria-invalid:focus:border-red-600 aria-invalid:focus:ring-red-600';

function useControl(id: string | undefined) {
  const field = useContext(FieldContext);
  return {
    id: id ?? field?.id,
    'aria-invalid': field?.invalid || undefined,
    'aria-describedby': field?.describedBy,
  };
}

/** Text input; use inside a {@link Field}. All `<input>` props pass through (`type`, `value`, `onChange`, `placeholder`…). */
export function Input({ className, id, ...props }: ComponentProps<'input'>) {
  return <input {...useControl(id)} className={cx(CONTROL, 'rounded-full px-4', className)} {...props} />;
}

/** Search box with a magnifying glass, for filtering a list; give it an `aria-label`. `className` sizes the box. */
export function SearchInput({ className, ...props }: ComponentProps<'input'>) {
  return (
    <div className={cx('relative min-w-0', className)}>
      <Icon
        name="magnifying-glass"
        className="pointer-events-none absolute inset-y-0 start-4 my-auto size-4 text-gray-400"
      />
      <Input type="search" className="ps-10 [&::-webkit-search-cancel-button]:hidden" {...props} />
    </div>
  );
}

/**
 * Native select; use inside a {@link Field}.
 *
 * ```tsx
 * <Select value={term} onChange={(e) => setTerm(e.target.value as Term)}>
 *   {TERMS.map((t) => <option key={t.value} value={t.value}>{t.label}</option>)}
 * </Select>
 * ```
 */
export function Select({ className, id, ...props }: ComponentProps<'select'>) {
  return <select {...useControl(id)} className={cx(CONTROL, 'rounded-full ps-4 pe-10', className)} {...props} />;
}

/** Multi-line input; use inside a {@link Field}. Defaults to 3 rows. Large radius, not a pill: a full round clips multi-line text. */
export function Textarea({ className, id, rows = 3, ...props }: ComponentProps<'textarea'>) {
  return <textarea {...useControl(id)} rows={rows} className={cx(CONTROL, 'rounded-3xl px-4', className)} {...props} />;
}

/** Props of {@link Checkbox} and {@link Toggle}. */
export interface CheckProps {
  label: ReactNode;
  description?: ReactNode;
  checked: boolean;
  onChange: (checked: boolean) => void;
  disabled?: boolean;
}

/**
 * Checkbox with label and optional description (HyperUI checkboxes, "Described").
 *
 * ```tsx
 * <Checkbox label="Returning resident" checked={returning} onChange={setReturning} />
 * ```
 */
export function Checkbox({ label, description, checked, onChange, disabled }: CheckProps) {
  return (
    <label className="flex items-start gap-3 has-disabled:cursor-not-allowed has-disabled:opacity-50">
      <input
        type="checkbox"
        checked={checked}
        disabled={disabled}
        onChange={(e) => onChange(e.target.checked)}
        className="my-0.5 size-4 rounded border-gray-300 text-primary-900 shadow-sm focus:ring-primary-900"
      />
      <span className="text-sm">
        <span className="font-medium text-gray-900">{label}</span>
        {description && (
          <span className="mt-0.5 block text-gray-600">{description}</span>
        )}
      </span>
    </label>
  );
}

/**
 * On/off switch (HyperUI toggles, "Base").
 *
 * ```tsx
 * <Toggle label="Print receipts" checked={print} onChange={setPrint} />
 * ```
 */
export function Toggle({ label, description, checked, onChange, disabled }: CheckProps) {
  return (
    <label className="flex items-start justify-between gap-4 has-disabled:cursor-not-allowed has-disabled:opacity-50">
      <span className="text-sm">
        <span className="font-medium text-gray-900">{label}</span>
        {description && (
          <span className="mt-0.5 block text-gray-600">{description}</span>
        )}
      </span>
      <span className="relative block h-6 w-11 shrink-0 rounded-full bg-gray-300 transition-colors [-webkit-tap-highlight-color:transparent] has-checked:bg-primary-900 has-focus-visible:outline-2 has-focus-visible:outline-offset-2 has-focus-visible:outline-primary-900">
        <input
          type="checkbox"
          role="switch"
          aria-checked={checked}
          checked={checked}
          disabled={disabled}
          onChange={(e) => onChange(e.target.checked)}
          className="peer sr-only"
        />
        <span className="absolute inset-y-0 start-0 m-0.5 size-5 rounded-full bg-white transition-[inset-inline-start] peer-checked:start-5" />
      </span>
    </label>
  );
}
