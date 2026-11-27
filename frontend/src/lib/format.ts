/**
 * Shared labels and date display, ported from main's ui.rs so every page
 * words things the same way. Money formatting lives in money.ts.
 */
import type { Term, PaymentMethod } from './types';

/** Terms of a stay, in form order. */
export const TERMS: { value: Term; label: string }[] = [
  { value: 'semester_1', label: 'Semester 1' },
  { value: 'semester_2', label: 'Semester 2' },
  { value: 'recess', label: 'Recess' },
];

/** Payment methods, in form order. */
export const METHODS: { value: PaymentMethod; label: string }[] = [
  { value: 'cash', label: 'Cash' },
  { value: 'mobile_money', label: 'Mobile money' },
  { value: 'bank_transfer', label: 'Bank transfer' },
  { value: 'card', label: 'Card' },
];

/** Date formats a hostel can pick (the `app_preferences` CHECK constraint). */
export const DATE_FORMATS = ['dd/MM/yyyy', 'MM/dd/yyyy', 'yyyy-MM-dd'] as const;

const PLANS: Record<string, string> = {
  trial: 'Free trial',
  semester: 'Semester plan',
  '1-year': '1-year plan',
  '3-year': '3-year plan',
  '5-year': '5-year plan',
};

/** `"semester_1"` → `"Semester 1"`. Unknown values are shown as given. */
export const termLabel = (term: string) => TERMS.find((t) => t.value === term)?.label ?? term;
/** `"mobile_money"` → `"Mobile money"`. */
export const methodLabel = (method: string) =>
  METHODS.find((m) => m.value === method)?.label ?? method;
/** `"1-year"` → `"1-year plan"`. */
export const planLabel = (plan: string) => PLANS[plan] ?? 'License';

const pad = (n: number) => String(n).padStart(2, '0');

/** A local calendar date as `"YYYY-MM-DD"` (what `NaiveDate` fields expect). */
export function isoDate(date: Date): string {
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}`;
}

/** Today's local date as `"YYYY-MM-DD"`, e.g. for `dashboard_stats`. */
export const todayIso = () => isoDate(new Date());

/** Academic years run August to July: March 2027 is still `"2026/2027"`. */
export function academicYear(today = new Date()): string {
  const start = today.getMonth() >= 7 ? today.getFullYear() : today.getFullYear() - 1;
  return `${start}/${start + 1}`;
}

/** Whole days from today (local) to a `"YYYY-MM-DD"` date; negative when it has passed. */
export function daysUntil(date: string): number {
  const [y = 0, m = 1, d = 1] = date.split('-').map(Number);
  const now = new Date();
  const today = Date.UTC(now.getFullYear(), now.getMonth(), now.getDate());
  return Math.round((Date.UTC(y, m - 1, d) - today) / 86_400_000);
}

function toLocalDate(value: string): Date | null {
  // A NaiveDate is a calendar day, not an instant: never shift it by time zone.
  const day = /^(\d{4})-(\d{2})-(\d{2})$/.exec(value);
  const date = day ? new Date(Number(day[1]), Number(day[2]) - 1, Number(day[3])) : new Date(value);
  return Number.isNaN(date.getTime()) ? null : date;
}

/**
 * Show a `NaiveDate` or RFC 3339 timestamp as a local date in the hostel's
 * `date_format` (`dd/MM/yyyy`, `MM/dd/yyyy` or `yyyy-MM-dd`). Pages normally
 * use `desk.formatDate`, which passes the format for them.
 */
export function formatDate(value: string, format: string): string {
  const date = toLocalDate(value);
  if (!date) return value;
  const [dd, mm, yyyy] = [pad(date.getDate()), pad(date.getMonth() + 1), String(date.getFullYear())];
  if (format === 'MM/dd/yyyy') return `${mm}/${dd}/${yyyy}`;
  if (format === 'yyyy-MM-dd') return `${yyyy}-${mm}-${dd}`;
  return `${dd}/${mm}/${yyyy}`;
}

/** `formatDate` plus the local time, `"27/11/2026 09:30"`. */
export function formatDateTime(value: string, format: string): string {
  const date = toLocalDate(value);
  if (!date) return value;
  return `${formatDate(value, format)} ${pad(date.getHours())}:${pad(date.getMinutes())}`;
}

/** Trimmed text, or `null` when blank: for optional request fields (`Option<String>`). */
export function blank(value: string): string | null {
  const trimmed = value.trim();
  return trimmed === '' ? null : trimmed;
}

/** Loose email check used by every form (Rust and the server validate again). */
export const looksLikeEmail = (value: string) => /^[^\s@]+@[^\s@]+$/.test(value.trim());
