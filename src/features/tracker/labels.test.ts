import { describe, expect, it } from 'vitest';
import { changeText, courseLabel } from './labels';

describe('courseLabel', () => {
  it('prefers the short name', () => {
    expect(courseLabel({ courseName: 'Álgebra Lineal', courseShortName: 'ALG' })).toBe('ALG');
  });

  it('falls back to the course name', () => {
    expect(courseLabel({ courseName: 'Álgebra Lineal', courseShortName: null })).toBe('Álgebra Lineal');
  });
});

describe('changeText', () => {
  it('describes a new item', () => {
    expect(changeText({ kind: 'new' }, null)).toBe('New');
  });

  it('describes a changed date', () => {
    expect(changeText({ kind: 'due_changed', previousDueAt: '2026-10-05T12:00:00Z' }, '2026-10-09T12:00:00Z')).toBe(
      'Date changed: 5 Oct → 9 Oct',
    );
  });
});

describe('changeText edge cases', () => {
  it('describes a date added where there was none', () => {
    expect(changeText({ kind: 'due_changed', previousDueAt: null }, '2026-10-09T12:00:00Z')).toBe(
      'Date changed: no date → 9 Oct',
    );
  });

  it('describes a date removed', () => {
    expect(changeText({ kind: 'due_changed', previousDueAt: '2026-10-05T12:00:00Z' }, null)).toBe(
      'Date changed: 5 Oct → no date',
    );
  });
});
