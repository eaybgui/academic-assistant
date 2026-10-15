import type { LmsCourseDto } from '../../lib/api/types';

export interface CourseSelectionListProps {
  courses: LmsCourseDto[];
  selected: ReadonlySet<number>;
  onChange: (next: Set<number>) => void;
}

const LINK_BUTTON = 'text-sm text-neutral-700 hover:underline';

/** Checkbox list used to choose which courses of a university are tracked. */
export default function CourseSelectionList({ courses, selected, onChange }: CourseSelectionListProps) {
  const sorted = [...courses].sort((a, b) => a.name.localeCompare(b.name));

  function toggle(id: number) {
    const next = new Set(selected);
    if (next.has(id)) next.delete(id);
    else next.add(id);
    onChange(next);
  }

  return (
    <fieldset className="flex flex-col gap-2">
      <legend className="mb-1 text-sm font-medium text-neutral-900">Courses</legend>
      <div className="flex gap-3">
        <button type="button" className={LINK_BUTTON} onClick={() => onChange(new Set(courses.map((c) => c.id)))}>
          Select all
        </button>
        <button type="button" className={LINK_BUTTON} onClick={() => onChange(new Set())}>
          Select none
        </button>
      </div>
      <ul className="flex flex-col gap-1">
        {sorted.map((course) => (
          <li key={course.id}>
            <label className="flex flex-wrap items-center gap-2 text-sm text-neutral-900">
              <input
                type="checkbox"
                checked={selected.has(course.id)}
                onChange={() => toggle(course.id)}
              />
              <span>{course.name}</span>
              {course.shortName && course.shortName !== course.name ? (
                <span className="text-neutral-500">{course.shortName}</span>
              ) : null}
              {course.isNew ? (
                <span className="rounded bg-neutral-900 px-1.5 py-0.5 text-xs font-medium text-white">New</span>
              ) : null}
              {!course.available ? <span className="text-neutral-500">Not visible on the platform</span> : null}
            </label>
          </li>
        ))}
      </ul>
    </fieldset>
  );
}
