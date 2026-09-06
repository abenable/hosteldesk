import {
  Children,
  isValidElement,
  type ChangeEvent,
  type OptionHTMLAttributes,
  type ReactNode,
} from 'react';
import * as SelectPrimitive from '@radix-ui/react-select';
import { CheckIcon, ChevronDownIcon } from './icons.tsx';

type Props = {
  label?: string;
  error?: string;
  /** When true, each option's text is Capitalized for display. */
  capitalize?: boolean;
  id?: string;
  name?: string;
  className?: string;
  value?: string;
  disabled?: boolean;
  required?: boolean;
  onChange?: (event: ChangeEvent<HTMLSelectElement>) => void;
  children: ReactNode;
};

type Option = { value: string; label: ReactNode; disabled?: boolean };

function collectOptions(children: ReactNode): Option[] {
  const options: Option[] = [];
  Children.forEach(children, (child) => {
    if (!isValidElement<OptionHTMLAttributes<HTMLOptionElement>>(child)) return;
    options.push({
      value: String(child.props.value ?? ''),
      label: child.props.children,
      disabled: child.props.disabled,
    });
  });
  return options;
}

export function Select({
  label,
  error,
  capitalize,
  id,
  name,
  className = '',
  children,
  value,
  disabled,
  required,
  onChange,
}: Props) {
  const selectId = id ?? name;
  const options = collectOptions(children);
  const placeholder = options.find((o) => o.value === '');
  const items = options.filter((o) => o.value !== '');

  return (
    <div className="flex flex-col gap-1">
      {label && (
        <label htmlFor={selectId} className="text-caption text-muted">
          {label}
        </label>
      )}
      <SelectPrimitive.Root
        value={value}
        disabled={disabled}
        required={required}
        name={name}
        onValueChange={(v) =>
          onChange?.({ target: { value: v } } as ChangeEvent<HTMLSelectElement>)
        }
      >
        <SelectPrimitive.Trigger
          id={selectId}
          className={`flex h-9 w-full items-center justify-between gap-2 rounded-full border bg-canvas px-4 text-body-sm text-ink focus:border-ink focus:outline-none data-[placeholder]:text-muted-soft ${
            error ? 'border-error-text' : 'border-hairline'
          } focus:border-2 ${capitalize ? 'capitalize' : ''} ${className}`}
        >
          <SelectPrimitive.Value placeholder={placeholder?.label} />
          <SelectPrimitive.Icon className="shrink-0 text-muted">
            <ChevronDownIcon width={16} height={16} />
          </SelectPrimitive.Icon>
        </SelectPrimitive.Trigger>
        <SelectPrimitive.Portal>
          <SelectPrimitive.Content
            position="popper"
            sideOffset={4}
            className="z-50 min-w-[var(--radix-select-trigger-width)] overflow-hidden rounded-md border border-hairline bg-canvas shadow-card"
          >
            <SelectPrimitive.Viewport className="p-1">
              {items.map((o) => (
                <SelectPrimitive.Item
                  key={o.value}
                  value={o.value}
                  disabled={o.disabled}
                  className={`flex cursor-pointer items-center justify-between gap-2 rounded-xs px-3 py-2 text-body-sm text-ink outline-none data-[highlighted]:bg-surface-soft data-[disabled]:cursor-not-allowed data-[disabled]:text-muted-soft ${
                    capitalize ? 'capitalize' : ''
                  }`}
                >
                  <SelectPrimitive.ItemText>{o.label}</SelectPrimitive.ItemText>
                  <SelectPrimitive.ItemIndicator className="text-ink">
                    <CheckIcon width={14} height={14} />
                  </SelectPrimitive.ItemIndicator>
                </SelectPrimitive.Item>
              ))}
            </SelectPrimitive.Viewport>
          </SelectPrimitive.Content>
        </SelectPrimitive.Portal>
      </SelectPrimitive.Root>
      {error && <p className="text-caption-sm text-error-text">{error}</p>}
    </div>
  );
}
