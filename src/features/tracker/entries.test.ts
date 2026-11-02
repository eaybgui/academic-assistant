import { describe, expect, it } from 'vitest';
import {
  compareEntries,
  entryDay,
  entryStatus,
  entryTitle,
  isEntryCompleted,
  localDayOf,
  personalEntry,
  personalStatus,
  platformEntry,
} from './entries';
import type { LmsItemDto, PersonalTaskDto } from '../../lib/api/types';

function task(date: string, overrides: Partial<PersonalTaskDto> = {}): PersonalTaskDto {
  return { id: 7, title: 'Go to the doctor', date, note: null, done: false, ...overrides };
}

function item(overrides: Partial<LmsItemDto> = {}): LmsItemDto {
  return {
    id: 3,
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

describe('tracker entries', () => {
  it('builds keys from the kind and id', () => {
    expect(platformEntry(item()).key).toBe('platform-3');
    expect(personalEntry(task('2026-11-05')).key).toBe('personal-7');
  });

  it('personal task dated today is pending at any hour', () => {
    expect(personalStatus(task('2026-11-05'), '2026-11-05')).toBe('pending');
  });

  it('undone task from yesterday is overdue', () => {
    expect(personalStatus(task('2026-11-04'), '2026-11-05')).toBe('overdue');
    expect(personalStatus(task('2026-11-04', { done: true }), '2026-11-05')).toBe('done');
  });

  it('localDayOf uses the local day', () => {
    expect(localDayOf(new Date(2026, 10, 5, 23, 30).toISOString())).toBe('2026-11-05');
    expect(localDayOf(new Date(2026, 10, 6, 0, 10).toISOString())).toBe('2026-11-06');
  });

  it('exposes status, title, day and completion for both kinds', () => {
    const platform = platformEntry(
      item({ status: 'submitted', dueAt: new Date(2026, 10, 5, 23, 30).toISOString() }),
    );
    expect(entryStatus(platform, '2026-11-05')).toBe('submitted');
    expect(entryTitle(platform)).toBe('Entrega 1');
    expect(entryDay(platform)).toBe('2026-11-05');
    expect(isEntryCompleted(platform, '2026-11-05')).toBe(true);
    expect(entryDay(platformEntry(item()))).toBeNull();

    const personal = personalEntry(task('2026-11-05', { done: true }));
    expect(entryStatus(personal, '2026-11-05')).toBe('done');
    expect(entryTitle(personal)).toBe('Go to the doctor');
    expect(entryDay(personal)).toBe('2026-11-05');
    expect(isEntryCompleted(personal, '2026-11-05')).toBe(true);
    expect(isEntryCompleted(personalEntry(task('2026-11-05')), '2026-11-05')).toBe(false);
  });

  it('personal task sorts before a timed deadline the same day', () => {
    const platform = platformEntry(item({ dueAt: new Date(2026, 10, 5, 9, 0).toISOString() }));
    const personal = personalEntry(task('2026-11-05'));
    expect([platform, personal].sort(compareEntries).map((e) => e.kind)).toEqual([
      'personal',
      'platform',
    ]);
  });

  it('sorts undated entries last and breaks ties on title then key', () => {
    const undated = platformEntry(item({ id: 1, title: 'Alpha' }));
    const dated = personalEntry(task('2026-11-05', { id: 2, title: 'Zeta' }));
    const sameTitleA = personalEntry(task('2026-11-06', { id: 3, title: 'Beta' }));
    const sameTitleB = personalEntry(task('2026-11-06', { id: 4, title: 'Beta' }));
    const sorted = [undated, sameTitleB, sameTitleA, dated].sort(compareEntries);
    expect(sorted.map((e) => e.key)).toEqual([
      'personal-2',
      'personal-3',
      'personal-4',
      'platform-1',
    ]);
  });
});
