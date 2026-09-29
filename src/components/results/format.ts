// Date/time formatting helpers shared by the dashboard, results and runs screens.
// All formatting uses `Intl.DateTimeFormat('en-GB')` per the design spec (§11).

const DATE_FORMATTER = new Intl.DateTimeFormat('en-GB', {
  day: 'numeric',
  month: 'short',
  year: 'numeric',
});

const DATE_TIME_FORMATTER = new Intl.DateTimeFormat('en-GB', {
  day: 'numeric',
  month: 'short',
  year: 'numeric',
  hour: '2-digit',
  minute: '2-digit',
});

const TIME_FORMATTER = new Intl.DateTimeFormat('en-GB', {
  hour: '2-digit',
  minute: '2-digit',
});

const SHORT_DATE_FORMATTER = new Intl.DateTimeFormat('en-GB', {
  day: 'numeric',
  month: 'short',
});

/** Pads a local date component (month or day) to two digits. */
function pad2(n: number): string {
  return String(n).padStart(2, '0');
}

/**
 * Today's date in the viewer's local timezone, as `YYYY-MM-DD`. Used to compare against a
 * result's `date` (also `YYYY-MM-DD`, no timezone) so "today" reflects the local calendar day
 * rather than UTC's.
 */
export function todayLocalIso(): string {
  const now = new Date();
  return `${now.getFullYear()}-${pad2(now.getMonth() + 1)}-${pad2(now.getDate())}`;
}

/** Formats an ISO timestamp as a date, e.g. "27 Oct 2026". */
export function formatDate(iso: string): string {
  return DATE_FORMATTER.format(new Date(iso));
}

/** Formats an ISO timestamp as a date and time, e.g. "27 Oct 2026, 10:00". */
export function formatDateTime(iso: string): string {
  return DATE_TIME_FORMATTER.format(new Date(iso));
}

/** Formats an ISO timestamp as a time, e.g. "09:14". */
export function formatTime(iso: string): string {
  return TIME_FORMATTER.format(new Date(iso));
}

/**
 * Formats a result date (`YYYY-MM-DD`) plus its optional `HH:MM` time, e.g.
 * "27 Oct" or "27 Oct 10:00". Used for the date chips on result cards/rows.
 */
export function formatResultDate(date: string, time: string | null): string {
  const formatted = SHORT_DATE_FORMATTER.format(new Date(`${date}T00:00:00`));
  return time ? `${formatted} ${time}` : formatted;
}

/**
 * Formats an ISO timestamp relative to today as "today", "yesterday", or a
 * short date. Used for the dashboard's last-check line.
 */
export function formatRelativeDay(iso: string): string {
  const date = new Date(iso);
  const now = new Date();
  const startOfDay = (d: Date) => new Date(d.getFullYear(), d.getMonth(), d.getDate()).getTime();
  const diffDays = Math.round((startOfDay(now) - startOfDay(date)) / 86_400_000);
  if (diffDays === 0) return 'today';
  if (diffDays === 1) return 'yesterday';
  return formatDate(iso);
}

/**
 * Formats the duration between two ISO timestamps, e.g. "2m 15s" or "45s".
 * Returns `null` while the run has not finished yet (`finishedAt` is `null`).
 */
export function formatDuration(startedAt: string, finishedAt: string | null): string | null {
  if (!finishedAt) return null;
  const ms = new Date(finishedAt).getTime() - new Date(startedAt).getTime();
  if (ms < 0) return null;
  const totalSeconds = Math.round(ms / 1000);
  const minutes = Math.floor(totalSeconds / 60);
  const seconds = totalSeconds % 60;
  return minutes > 0 ? `${minutes}m ${seconds}s` : `${seconds}s`;
}
