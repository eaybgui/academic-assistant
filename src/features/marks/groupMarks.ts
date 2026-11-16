import type { LmsItemDto, TrackerDto } from '../../lib/api/types';
import { compareEntries, platformEntry } from '../tracker/entries';

export interface CourseMarks {
  courseId: number;
  courseName: string;
  courseShortName: string | null;
  items: LmsItemDto[];
}

export interface UniversityMarks {
  universityId: number;
  universityName: string;
  courses: CourseMarks[];
}

const COLLATOR = new Intl.Collator(undefined, { sensitivity: 'base', numeric: true });

function compareItems(a: LmsItemDto, b: LmsItemDto): number {
  return compareEntries(platformEntry(a), platformEntry(b));
}

function compareCourses(a: CourseMarks, b: CourseMarks): number {
  return COLLATOR.compare(a.courseName, b.courseName) || a.courseId - b.courseId;
}

function groupCourses(items: LmsItemDto[]): CourseMarks[] {
  const byCourse = new Map<number, CourseMarks>();
  for (const item of items) {
    const course = byCourse.get(item.courseId) ?? {
      courseId: item.courseId,
      courseName: item.courseName,
      courseShortName: item.courseShortName,
      items: [],
    };
    course.items.push(item);
    byCourse.set(item.courseId, course);
  }
  const courses = [...byCourse.values()];
  for (const course of courses) course.items.sort(compareItems);
  return courses.sort(compareCourses);
}

/** The graded items, grouped by university (in the given order) and then by course; groups without marks are left out. */
export function groupMarks(data: Pick<TrackerDto, 'items' | 'universities'>): UniversityMarks[] {
  const graded = data.items.filter((item) => item.grade !== null);
  return data.universities.flatMap((university) => {
    const courses = groupCourses(graded.filter((item) => item.universityId === university.id));
    return courses.length === 0
      ? []
      : [{ universityId: university.id, universityName: university.name, courses }];
  });
}
