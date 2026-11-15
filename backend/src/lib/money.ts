/**
 * Currency parsing and validation.
 *
 * Port of `hosteldesk-core`'s `parse_money` / `parse_positive_money`.
 *
 * Money is represented end-to-end as a decimal *string*. It is never parsed
 * into a float: `number` cannot represent most decimal values exactly, and
 * all aggregation (sums, balances) is done by Postgres `NUMERIC` inside the
 * database. This module only validates the textual form and canonicalizes
 * whitespace.
 */

/** Machine-readable reason a monetary string was rejected. */
export type MoneyErrorCode = "invalid_decimal" | "negative" | "zero";

export interface MoneyError {
  code: MoneyErrorCode;
  message: string;
}

/** Discriminated result: `{ ok: true, value }` with the canonical string, or an error. */
export type MoneyResult = { ok: true; value: string } | { ok: false; error: MoneyError };

/**
 * Optional sign, then digits with at most one decimal point. At least one
 * digit must be present on one side of the point. This mirrors what
 * `rust_decimal::Decimal::from_str` accepts plus what the SQLite CHECK
 * constraints guaranteed: unsigned-decimal text.
 */
const MONEY_PATTERN = /^[+-]?(?:\d+(?:\.\d+)?|\.\d+)$/;

/** Matches strings whose value is exactly zero ("0", "00.000", ".0", …). */
const ZERO_PATTERN = /^0*(?:\.0*)?$/;

function fail(code: MoneyErrorCode, message: string): MoneyResult {
  return { ok: false, error: { code, message } };
}

/**
 * Parse a non-negative currency amount from a string.
 *
 * Leading and trailing whitespace is ignored. Returns an error if the input
 * is empty, malformed, or negative. Zero is accepted; callers that require a
 * strictly positive amount should use {@link parsePositiveMoney}.
 */
export function parseMoney(input: string): MoneyResult {
  const raw = input.trim();
  if (!MONEY_PATTERN.test(raw)) {
    return fail("invalid_decimal", "amount must be a valid decimal amount");
  }
  if (raw.startsWith("-")) {
    return fail("negative", "amount must not be negative");
  }
  // Drop an explicit "+" so callers receive a canonical unsigned string.
  return { ok: true, value: raw.replace(/^\+/, "") };
}

/**
 * Parse a strictly positive currency amount from a string.
 *
 * Use this for rates and payment amounts, both of which must be greater than
 * zero.
 */
export function parsePositiveMoney(input: string): MoneyResult {
  const result = parseMoney(input);
  if (!result.ok) {
    return result;
  }
  if (ZERO_PATTERN.test(result.value)) {
    return fail("zero", "amount must be greater than zero");
  }
  return result;
}
