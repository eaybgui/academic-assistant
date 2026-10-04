import { describe, expect, it } from 'vitest';
import {
  formatDate,
  formatDateTime,
  formatDuration,
  formatRelativeDay,
  formatResultDate,
  formatTime,
  todayLocalIso,
} from './format';

describe('formatDate', () => {
  it('formats an ISO timestamp as en-GB day month year', () => {
    expect(formatDate('2026-10-27T12:00:00Z')).toMatch(/27 Oct 2026/);
  });
});

describe('formatDateTime', () => {
  it('includes both date and time', () => {
    expect(formatDateTime('2026-10-28T12:14:00Z')).toMatch(/28 Oct 2026/);
    expect(formatDateTime('2026-10-28T12:14:00Z')).toMatch(/\d{2}:\d{2}/);
  });
});

describe('formatTime', () => {
  it('formats as HH:MM', () => {
    expect(formatTime('2026-09-28T09:14:00Z')).toMatch(/^\d{2}:\d{2}$/);
  });
});

describe('formatResultDate', () => {
  it('omits the time when absent', () => {
    expect(formatResultDate('2026-10-27', null)).toBe('27 Oct');
  });

  it('appends the time when present', () => {
    expect(formatResultDate('2026-10-27', '10:00')).toBe('27 Oct 10:00');
  });
});

describe('formatRelativeDay', () => {
  it('returns "today" for the current moment', () => {
    expect(formatRelativeDay(new Date().toISOString())).toBe('today');
  });

  it('returns "yesterday" for exactly one day ago', () => {
    const yesterday = new Date();
    yesterday.setDate(yesterday.getDate() - 1);
    expect(formatRelativeDay(yesterday.toISOString())).toBe('yesterday');
  });

  it('returns a short date further in the past', () => {
    const longAgo = new Date();
    longAgo.setDate(longAgo.getDate() - 10);
    expect(formatRelativeDay(longAgo.toISOString())).toBe(formatDate(longAgo.toISOString()));
  });
});

describe('formatDuration', () => {
  it('returns null while the run has not finished', () => {
    expect(formatDuration('2026-09-28T09:00:00Z', null)).toBeNull();
  });

  it('formats minutes and seconds', () => {
    expect(formatDuration('2026-09-28T09:00:00Z', '2026-09-28T09:02:15Z')).toBe('2m 15s');
  });

  it('formats seconds only under a minute', () => {
    expect(formatDuration('2026-09-28T09:00:00Z', '2026-09-28T09:00:45Z')).toBe('45s');
  });
});

describe('todayLocalIso', () => {
  it('returns the local calendar date as YYYY-MM-DD, not the UTC one', () => {
    const now = new Date();
    const expected = `${now.getFullYear()}-${String(now.getMonth() + 1).padStart(2, '0')}-${String(
      now.getDate(),
    ).padStart(2, '0')}`;
    expect(todayLocalIso()).toBe(expected);
    expect(todayLocalIso()).toMatch(/^\d{4}-\d{2}-\d{2}$/);
  });
});
