function splitDecimal(s: string): [string, string] {
  const trimmed = (s || '0').trim();
  const [intPart, fracPart = ''] = trimmed.split('.');
  return [intPart || '0', fracPart];
}

export function addDecimalStrings(a: string, b: string): string {
  const [ia, fa] = splitDecimal(a);
  const [ib, fb] = splitDecimal(b);
  const len = Math.max(fa.length, fb.length);
  const va = BigInt(ia) * 10n ** BigInt(len) + BigInt(fa.padEnd(len, '0') || '0');
  const vb = BigInt(ib) * 10n ** BigInt(len) + BigInt(fb.padEnd(len, '0') || '0');
  const sum = va + vb;
  const int = sum / 10n ** BigInt(len);
  const frac = sum % 10n ** BigInt(len);
  return len > 0 ? `${int}.${frac.toString().padStart(len, '0')}` : `${int}`;
}

export function subtractDecimalStrings(a: string, b: string): string {
  const [ia, fa] = splitDecimal(a);
  const [ib, fb] = splitDecimal(b);
  const len = Math.max(fa.length, fb.length);
  const va = BigInt(ia) * 10n ** BigInt(len) + BigInt(fa.padEnd(len, '0') || '0');
  const vb = BigInt(ib) * 10n ** BigInt(len) + BigInt(fb.padEnd(len, '0') || '0');
  const difference = va - vb;
  const negative = difference < 0n;
  const magnitude = negative ? -difference : difference;
  const int = magnitude / 10n ** BigInt(len);
  const frac = magnitude % 10n ** BigInt(len);
  const body = len > 0 ? `${int}.${frac.toString().padStart(len, '0')}` : `${int}`;
  return negative ? `-${body}` : body;
}

export function formatDecimal(amount: string): string {
  const trimmed = (amount || '0').trim();
  const neg = trimmed.startsWith('-');
  const positive = neg ? trimmed.slice(1) : trimmed;
  const [intPart, fracPart] = positive.split('.');
  const withCommas = (intPart || '0').replace(/\B(?=(\d{3})+(?!\d))/g, ',');
  const body = fracPart !== undefined ? `${withCommas}.${fracPart}` : withCommas;
  return neg ? `-${body}` : body;
}

export function isPositiveDecimal(s: string): boolean {
  const trimmed = s.trim();
  return /^\d+(\.\d+)?$/.test(trimmed) && BigInt(trimmed.replace('.', '')) > 0n;
}
