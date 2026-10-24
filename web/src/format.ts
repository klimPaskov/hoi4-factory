// Dates and numbers always go through Intl with the browser's own locale, never a fixed one.

function locale(): string | undefined {
  if (typeof navigator === 'undefined') return undefined;
  return navigator.languages?.[0] ?? navigator.language ?? undefined;
}

function toDate(value: string | Date): Date | null {
  const date = value instanceof Date ? value : new Date(value);
  return Number.isNaN(date.getTime()) ? null : date;
}

export function formatDate(value: string | Date): string {
  const date = toDate(value);
  return date ? new Intl.DateTimeFormat(locale(), { dateStyle: 'medium' }).format(date) : '';
}

export function formatDateTime(value: string | Date): string {
  const date = toDate(value);
  return date
    ? new Intl.DateTimeFormat(locale(), { dateStyle: 'medium', timeStyle: 'short' }).format(date)
    : '';
}

export function formatNumber(value: number): string {
  return new Intl.NumberFormat(locale()).format(value);
}

/** "in 12 days", "tomorrow", in the browser language. */
export function formatDaysUntil(value: string | Date, now: Date = new Date()): string {
  const date = toDate(value);
  if (!date) return '';
  const days = Math.max(0, Math.ceil((date.getTime() - now.getTime()) / 86_400_000));
  return new Intl.RelativeTimeFormat(locale(), { numeric: 'auto' }).format(days, 'day');
}

const UNITS = ['byte', 'kilobyte', 'megabyte', 'gigabyte'] as const;

export function formatBytes(bytes: number): string {
  let value = bytes;
  let unit = 0;
  while (value >= 1000 && unit < UNITS.length - 1) {
    value /= 1000;
    unit += 1;
  }
  return new Intl.NumberFormat(locale(), {
    style: 'unit',
    unit: UNITS[unit],
    unitDisplay: 'short',
    maximumFractionDigits: unit === 0 ? 0 : 1,
  }).format(value);
}
