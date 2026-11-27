/**
 * Money display. Amounts are decimal strings from Rust; this module only
 * formats and inspects the string. It never converts money to a number, and
 * there is no arithmetic on money anywhere in the frontend (Rust computes
 * balances with Decimal).
 */

const DECIMAL = /^(-?)(\d+)(?:\.(\d+))?$/;

/**
 * `formatMoney("1500000.00", "UGX")` → `"UGX 1,500,000"`;
 * `formatMoney("12.5", "USD")` → `"USD 12.50"`.
 * Groups the integer part, drops an all-zero fraction, pads a real one to two
 * digits. Anything that is not a plain decimal is shown as given.
 */
export function formatMoney(amount: string, currency: string): string {
  const match = DECIMAL.exec(amount.trim());
  if (!match) return `${currency} ${amount}`.trim();
  const [, sign = '', whole = '0', fraction = ''] = match;
  const grouped = whole.replace(/^0+(?=\d)/, '').replace(/\B(?=(\d{3})+(?!\d))/g, ',');
  const cents = /^0*$/.test(fraction) ? '' : `.${fraction.padEnd(2, '0')}`;
  return `${currency} ${sign}${grouped}${cents}`;
}

/** True when a balance string is more than zero: `"0"`, `"0.00"`, `""` and credits (`"-5"`) are not owing. */
export function isOwing(balance: string): boolean {
  const value = balance.trim();
  return value !== '' && !value.startsWith('-') && !/^0*(\.0*)?$/.test(value);
}

const GROUPED = /^\d{1,3}(,\d{3})+(\.\d+)?$/;

/**
 * A typed amount as Rust expects it: trimmed, with thousands commas removed —
 * but only when they group correctly (`"1,500,000"` → `"1500000"`). Anything
 * else is returned trimmed and fails {@link isMoneyInput} (`"1,50"` is not 150).
 */
export function cleanMoney(value: string): string {
  const trimmed = value.trim();
  return GROUPED.test(trimmed) ? trimmed.replace(/,/g, '') : trimmed;
}

/**
 * Form check for a typed amount or rate: a positive decimal (`"1500"`, `"1,500"`,
 * `"1500.50"`), the same rule as `protocol::validate_money` once {@link cleanMoney}
 * has removed the grouping. Send `cleanMoney(value)`; Rust re-validates.
 */
export function isMoneyInput(value: string): boolean {
  const clean = cleanMoney(value);
  return /^\d+(\.\d+)?$/.test(clean) && isOwing(clean);
}
