import { formatDecimal } from '../../lib/decimal.ts';

type Props = {
  amount: string;
  currency: string;
  className?: string;
  muted?: boolean;
  error?: boolean;
};

export function CurrencyAmount({ amount, currency, className = '', muted = false, error = false }: Props) {
  return (
    <span
      className={`${error ? 'text-error-text' : muted ? 'text-muted' : 'text-ink'} ${className}`}
    >
      {currency} {formatDecimal(amount)}
    </span>
  );
}
