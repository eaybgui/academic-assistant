import { describe, expect, it } from 'vitest';
import type { GradeDto, LmsItemDto, LmsUniversityDto } from '../../lib/api/types';
import { groupMarks } from './groupMarks';

function university(id: number, name: string): LmsUniversityDto {
  return {
    id,
    name,
    platform: 'moodle',
    baseUrl: 'https://lms.example',
    loginState: 'ok',
    accountLabel: null,
    lastSync: { at: null, outcome: null, error: null },
    courses: { tracked: 1, total: 1, unreviewed: 0 },
  };
}

const GRADE: GradeDto = { display: '8 / 10', score: 8, max: 10 };

function item(id: number, overrides: Partial<LmsItemDto> = {}): LmsItemDto {
  return {
    id,
    universityId: 1,
    universityName: 'Universidad Demo',
    courseId: 10,
    courseName: 'Álgebra Lineal',
    courseShortName: null,
    externalId: `_${id}_1`,
    kind: 'assignment',
    title: `Práctica ${id}`,
    dueAt: null,
    url: null,
    status: 'graded',
    grade: GRADE,
    change: null,
    removedAt: null,
    done: false,
    hidden: false,
    note: null,
    ...overrides,
  };
}

const A = university(1, 'Universidad Demo');
const B = university(2, 'Universidad Muestra');

describe('groupMarks', () => {
  it('keeps only graded items grouped by university then course', () => {
    const items = [
      item(1, { universityId: 1 }),
      item(2, { universityId: 2, courseId: 20, courseName: 'Física' }),
      item(3, { universityId: 2, courseId: 20, courseName: 'Física', grade: null }),
    ];

    const result = groupMarks({ items, universities: [B, A] });

    expect(result.map((u) => u.universityId)).toEqual([2, 1]);
    expect(result[0].universityName).toBe('Universidad Muestra');
    expect(result[0].courses.map((c) => c.items.map((i) => i.id))).toEqual([[2]]);
    expect(result[1].courses.map((c) => c.items.map((i) => i.id))).toEqual([[1]]);
  });

  it('omits universities and courses without marks', () => {
    const items = [
      item(1, { universityId: 1, grade: null }),
      item(2, { universityId: 2, courseId: 20, courseName: 'Física' }),
      item(3, { universityId: 2, courseId: 21, courseName: 'Química', grade: null }),
    ];

    const result = groupMarks({ items, universities: [A, B] });

    expect(result.map((u) => u.universityId)).toEqual([2]);
    expect(result[0].courses.map((c) => c.courseId)).toEqual([20]);
  });

  it('sorts courses by locale-aware name', () => {
    const names = ['Zoología', 'álgebra', 'Biología'];
    const items = names.map((courseName, index) =>
      item(index + 1, { courseId: index + 1, courseName }),
    );

    const result = groupMarks({ items, universities: [A] });

    expect(result[0].courses.map((c) => c.courseName)).toEqual(['álgebra', 'Biología', 'Zoología']);
  });

  it('sorts items by due date, undated last', () => {
    const items = [
      item(1, { dueAt: '2026-03-01T10:00:00Z' }),
      item(2, { dueAt: null }),
      item(3, { dueAt: '2026-01-15T10:00:00Z' }),
    ];

    const result = groupMarks({ items, universities: [A] });

    expect(result[0].courses[0].items.map((i) => i.id)).toEqual([3, 1, 2]);
  });
});
