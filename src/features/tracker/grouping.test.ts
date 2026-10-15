import { describe, expect, it } from 'vitest';
import { compareItems, groupByStatus, groupByTime, isCompleted } from './grouping';
import type { LmsItemDto } from '../../lib/api/types';

function item(overrides: Partial<LmsItemDto> = {}): LmsItemDto {
  return {
    id: 1,
    universityId: 1,
    universityName: 'Universidad Demo',
    courseId: 1,
    courseName: 'Álgebra Lineal',
    courseShortName: null,
    externalId: '_10_1',
    kind: 'assignment',
    title: 'Entrega 1',
    dueAt: null,
    url: null,
    status: 'pending',
    grade: null,
    change: null,
    removedAt: null,
    done: false,
    hidden: false,
    note: null,
    ...overrides,
  };
}

const now = new Date(2026, 9, 7, 10, 0); // Wednesday 7 Oct 2026, 10:00 local

describe('tracker grouping', () => {
  it('completed items are submitted, graded or past events', () => {
    expect(isCompleted(item({ status: 'submitted' }))).toBe(true);
    expect(isCompleted(item({ status: 'graded' }))).toBe(true);
    expect(isCompleted(item({ status: 'past_event' }))).toBe(true);
    expect(isCompleted(item({ status: 'overdue' }))).toBe(false);
    expect(isCompleted(item({ status: 'pending' }))).toBe(false);
  });

  it('groups open items by local time, weeks starting on Monday', () => {
    const at = (d: Date) => d.toISOString();
    const items = [
      item({ id: 1, status: 'overdue', dueAt: at(new Date(2026, 9, 6, 23, 59)) }),
      item({ id: 2, status: 'pending', dueAt: at(new Date(2026, 9, 7, 23, 30)) }), // today
      item({ id: 3, status: 'pending', dueAt: at(new Date(2026, 9, 11, 23, 30)) }), // Sunday → this week
      item({ id: 4, status: 'pending', dueAt: at(new Date(2026, 9, 12, 0, 10)) }), // Monday → next week
      item({ id: 5, status: 'pending', dueAt: at(new Date(2026, 9, 18, 23, 0)) }), // next Sunday → next week
      item({ id: 6, status: 'pending', dueAt: at(new Date(2026, 9, 19, 9, 0)) }), // later
      item({ id: 7, status: 'pending', dueAt: null }), // no date
      item({ id: 8, status: 'graded', dueAt: at(new Date(2026, 9, 1, 9, 0)) }), // completed
    ];
    const { groups, completed } = groupByTime(items, now);
    const ids = Object.fromEntries(groups.map((g) => [g.key, g.items.map((i) => i.id)]));
    expect(ids).toEqual({
      overdue: [1],
      today: [2],
      this_week: [3],
      next_week: [4, 5],
      later: [6],
      no_date: [7],
    });
    expect(completed.map((i) => i.id)).toEqual([8]);
    expect(groups.map((g) => g.label)).toEqual([
      'Overdue',
      'Today',
      'This week',
      'Next week',
      'Later',
      'No date',
    ]);
  });

  it('omits empty groups', () => {
    const { groups } = groupByTime([item({ status: 'pending', dueAt: null })], now);
    expect(groups.map((g) => g.key)).toEqual(['no_date']);
  });

  it('an open item due earlier today that the backend still calls pending stays in Today', () => {
    const { groups } = groupByTime(
      [item({ status: 'pending', dueAt: new Date(2026, 9, 7, 9, 0).toISOString() })],
      now,
    );
    expect(groups[0].key).toBe('today');
  });

  it('groups by status with overdue first inside pending', () => {
    const items = [
      item({ id: 1, status: 'pending', dueAt: '2026-10-09T10:00:00Z' }),
      item({ id: 2, status: 'overdue', dueAt: '2026-10-05T10:00:00Z' }),
      item({ id: 3, status: 'submitted' }),
      item({ id: 4, status: 'graded' }),
      item({ id: 5, status: 'past_event' }),
    ];
    const { groups, completed } = groupByStatus(items);
    expect(groups.map((g) => [g.key, g.items.map((i) => i.id)])).toEqual([
      ['pending', [2, 1]],
      ['submitted', [3]],
      ['graded', [4]],
    ]);
    expect(completed.map((i) => i.id)).toEqual([5]); // past events collapsed like "Completed"
  });

  it('sorts by due date, undated last, then title with a locale-aware compare', () => {
    const a = item({ id: 1, title: 'Zeta', dueAt: null });
    const b = item({ id: 2, title: 'álgebra', dueAt: null });
    const c = item({ id: 3, title: 'Beta', dueAt: '2026-10-08T10:00:00Z' });
    expect([a, b, c].sort(compareItems).map((i) => i.id)).toEqual([3, 2, 1]);
  });
});
