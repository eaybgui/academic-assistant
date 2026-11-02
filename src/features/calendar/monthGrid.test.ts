import { describe, expect, it } from 'vitest';
import type { LmsItemDto, PersonalTaskDto } from '../../lib/api/types';
import { personalEntry, platformEntry } from '../tracker/entries';
import { dotTone, entriesByDay, monthGrid } from './monthGrid';

function item(overrides: Partial<LmsItemDto> = {}): LmsItemDto {
  return {
    id: 7,
    universityId: 1,
    universityName: 'Universidad Demo',
    courseId: 3,
    courseName: 'Álgebra Lineal',
    courseShortName: null,
    externalId: '_10_1',
    kind: 'assignment',
    title: 'Hoja de problemas 1',
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
  return { id: 5, title: 'Go to the doctor', date: '2026-11-05', note: null, done: false, ...overrides };
}

describe('monthGrid', () => {
  it('covers November 2026 in 6 Monday-first weeks', () => {
    const weeks = monthGrid(2026, 10);

    expect(weeks).toHaveLength(6);
    expect(weeks.every((week) => week.length === 7)).toBe(true);
    expect(weeks[0][0]).toEqual({ date: '2026-10-26', inMonth: false });
    expect(weeks[0][6]).toEqual({ date: '2026-11-01', inMonth: true });
    expect(weeks[5][6]).toEqual({ date: '2026-12-06', inMonth: false });
  });

  it('uses exactly 4 weeks for February 2027', () => {
    const weeks = monthGrid(2027, 1);

    expect(weeks).toHaveLength(4);
    expect(weeks[0][0]).toEqual({ date: '2027-02-01', inMonth: true });
    expect(weeks[3][6]).toEqual({ date: '2027-02-28', inMonth: true });
    expect(weeks.flat().every((day) => day.inMonth)).toBe(true);
  });
});

describe('entriesByDay', () => {
  it('places an item on its local due day and omits undated items', () => {
    const dated = platformEntry(item({ id: 1, dueAt: new Date(2026, 10, 5, 23, 30).toISOString() }));
    const undated = platformEntry(item({ id: 2, dueAt: null }));

    const byDay = entriesByDay([dated, undated]);

    expect(byDay.get('2026-11-05')).toEqual([dated]);
    expect(byDay.size).toBe(1);
  });

  it('sorts each day with compareEntries', () => {
    const later = personalEntry(task({ id: 1, title: 'Buy a gift' }));
    const earlier = personalEntry(task({ id: 2, title: 'Ask for a reference' }));

    expect(entriesByDay([later, earlier]).get('2026-11-05')).toEqual([earlier, later]);
  });
});

describe('dotTone', () => {
  const today = '2026-11-03';

  it('maps personal tasks to personal or personal_done', () => {
    expect(dotTone(personalEntry(task({ done: true })), today)).toBe('personal_done');
    expect(dotTone(personalEntry(task()), today)).toBe('personal');
    expect(dotTone(personalEntry(task({ date: '2026-11-01' })), today)).toBe('personal');
  });

  it('maps platform items by status', () => {
    expect(dotTone(platformEntry(item({ status: 'graded' })), today)).toBe('completed');
    expect(dotTone(platformEntry(item({ status: 'submitted' })), today)).toBe('completed');
    expect(dotTone(platformEntry(item({ status: 'overdue' })), today)).toBe('overdue');
    expect(dotTone(platformEntry(item({ status: 'pending' })), today)).toBe('pending');
    expect(dotTone(platformEntry(item({ status: 'past_event' })), today)).toBe('past');
  });
});
