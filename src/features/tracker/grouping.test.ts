import { describe, expect, it } from 'vitest';
import {
  compareEntries,
  isEntryCompleted,
  personalEntry,
  platformEntry,
  type TrackerEntry,
} from './entries';
import { groupByStatus, groupByTime } from './grouping';
import type { LmsItemDto, PersonalTaskDto } from '../../lib/api/types';

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

function task(overrides: Partial<PersonalTaskDto> = {}): PersonalTaskDto {
  return { id: 1, title: 'Go to the doctor', date: '2026-11-04', note: null, done: false, ...overrides };
}

const entry = (overrides: Partial<LmsItemDto> = {}) => platformEntry(item(overrides));
const idOf = (e: TrackerEntry) => (e.kind === 'platform' ? e.item.id : e.task.id);

const now = new Date(2026, 9, 7, 10, 0); // Wednesday 7 Oct 2026, 10:00 local
const today = '2026-10-07';

describe('tracker grouping', () => {
  it('completed items are submitted, graded or past events', () => {
    expect(isEntryCompleted(entry({ status: 'submitted' }), today)).toBe(true);
    expect(isEntryCompleted(entry({ status: 'graded' }), today)).toBe(true);
    expect(isEntryCompleted(entry({ status: 'past_event' }), today)).toBe(true);
    expect(isEntryCompleted(entry({ status: 'overdue' }), today)).toBe(false);
    expect(isEntryCompleted(entry({ status: 'pending' }), today)).toBe(false);
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
    const { groups, completed } = groupByTime(items.map(platformEntry), now);
    const ids = Object.fromEntries(groups.map((g) => [g.key, g.items.map(idOf)]));
    expect(ids).toEqual({
      overdue: [1],
      today: [2],
      this_week: [3],
      next_week: [4, 5],
      later: [6],
      no_date: [7],
    });
    expect(completed.map(idOf)).toEqual([8]);
    expect(groups.map((g) => g.label)).toEqual([
      'Overdue',
      'Today',
      'This week',
      'Next week',
      'Later',
      'No date',
    ]);
  });

  it('on a Sunday, later that day is today and Monday is already next week', () => {
    const sunday = new Date(2026, 9, 11, 10, 0);
    const { groups } = groupByTime(
      [
        entry({ id: 1, status: 'pending', dueAt: new Date(2026, 9, 11, 23, 0).toISOString() }),
        entry({ id: 2, status: 'pending', dueAt: new Date(2026, 9, 12, 0, 10).toISOString() }),
      ],
      sunday,
    );
    const ids = Object.fromEntries(groups.map((g) => [g.key, g.items.map(idOf)]));
    expect(ids).toEqual({ today: [1], next_week: [2] });
  });

  it('omits empty groups', () => {
    const { groups } = groupByTime([entry({ status: 'pending', dueAt: null })], now);
    expect(groups.map((g) => g.key)).toEqual(['no_date']);
  });

  it('an open item due earlier today that the backend still calls pending stays in Today', () => {
    const { groups } = groupByTime(
      [entry({ status: 'pending', dueAt: new Date(2026, 9, 7, 9, 0).toISOString() })],
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
    const { groups, completed } = groupByStatus(items.map(platformEntry), now);
    expect(groups.map((g) => [g.key, g.items.map(idOf)])).toEqual([
      ['pending', [2, 1]],
      ['submitted', [3]],
      ['graded', [4]],
    ]);
    expect(completed.map(idOf)).toEqual([5]); // past events collapsed like "Completed"
  });

  it('sorts by due date, undated last, then title with a locale-aware compare', () => {
    const a = item({ id: 1, title: 'Zeta', dueAt: null });
    const b = item({ id: 2, title: 'álgebra', dueAt: null });
    const c = item({ id: 3, title: 'Beta', dueAt: '2026-10-08T10:00:00Z' });
    expect([a, b, c].map(platformEntry).sort(compareEntries).map(idOf)).toEqual([3, 2, 1]);
  });

  describe('with personal tasks', () => {
    const wednesday = new Date(2026, 10, 4, 10, 0); // Wednesday 4 Nov 2026
    const personal = (id: number, overrides: Partial<PersonalTaskDto> = {}) =>
      personalEntry(task({ id, ...overrides }));

    it('places them by their calendar day and never in No date', () => {
      const { groups, completed } = groupByTime(
        [
          personal(1, { date: '2026-11-03' }),
          personal(2, { date: '2026-11-04' }),
          personal(3, { date: '2026-11-08' }), // Sunday
          personal(4, { date: '2026-11-09' }), // Monday
          personal(5, { date: '2026-11-16' }),
          personal(6, { date: '2026-11-04', done: true }),
        ],
        wednesday,
      );
      const ids = Object.fromEntries(groups.map((g) => [g.key, g.items.map(idOf)]));
      expect(ids).toEqual({
        overdue: [1],
        today: [2],
        this_week: [3],
        next_week: [4],
        later: [5],
      });
      expect(completed.map(idOf)).toEqual([6]);
    });

    it('groups done tasks under Done, after Graded, and keeps overdue ones in Pending', () => {
      const { groups, completed } = groupByStatus(
        [
          personal(1, { date: '2026-11-03' }),
          personal(2, { done: true }),
          entry({ id: 3, status: 'graded' }),
          entry({ id: 4, status: 'pending', dueAt: new Date(2026, 10, 5, 9, 0).toISOString() }),
        ],
        wednesday,
      );
      expect(groups.map((g) => [g.key, g.label, g.items.map((e) => e.key)])).toEqual([
        ['pending', 'Pending', ['personal-1', 'platform-4']],
        ['graded', 'Graded', ['platform-3']],
        ['done', 'Done', ['personal-2']],
      ]);
      expect(completed).toEqual([]);
    });
  });
});
