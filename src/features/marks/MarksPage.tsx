import { useMemo } from 'react';
import { useNavigate } from 'react-router';
import { useQuery } from '@tanstack/react-query';
import ErrorMessage from '../../components/ErrorMessage';
import { formatDate } from '../../components/results/format';
import {
  CARD_ROW,
  PAGE_TITLE,
  PRIMARY_BUTTON,
  SECTION_TITLE,
  SUBSECTION_TITLE,
  TEXT_MUTED,
} from '../../components/styles';
import { api, toCommandError } from '../../lib/api/client';
import { queryKeys } from '../../lib/api/queryKeys';
import type { LmsItemDto, TrackerQuery } from '../../lib/api/types';
import CollapsibleSummary from '../tracker/CollapsibleSummary';
import EntryDetailsModal from '../tracker/EntryDetailsModal';
import { platformEntry, type TrackerEntry } from '../tracker/entries';
import { KIND_LABELS } from '../tracker/labels';
import { META_ROW_CLASS, TITLE_BUTTON_CLASS } from '../tracker/rowStyles';
import { useOpenEntry } from '../tracker/useOpenEntry';
import { groupMarks, type CourseMarks } from './groupMarks';

/** Hidden items still count as marks; items removed from the platform don't. */
const MARKS_QUERY: TrackerQuery = { universityId: null, includeHidden: true, includeRemoved: false };

function MarkRow({ item, onOpen }: { item: LmsItemDto; onOpen: () => void }) {
  return (
    <li className={CARD_ROW}>
      <div className="flex items-start justify-between gap-3">
        <button type="button" title={item.title} onClick={onOpen} className={TITLE_BUTTON_CLASS}>
          {item.title}
        </button>
        <span className="shrink-0 text-sm font-semibold text-neutral-900">{item.grade?.display}</span>
      </div>
      <div className={META_ROW_CLASS}>
        <span>{KIND_LABELS[item.kind]}</span>
        <span>{item.dueAt ? formatDate(item.dueAt) : 'No date'}</span>
      </div>
    </li>
  );
}

function CourseSection({ course, onOpen }: { course: CourseMarks; onOpen: (entry: TrackerEntry) => void }) {
  const showShortName =
    course.courseShortName !== null && course.courseShortName !== course.courseName;
  return (
    <details open className="group flex flex-col gap-2">
      <CollapsibleSummary>
        <h3 className={SUBSECTION_TITLE}>
          {course.courseName}
          {showShortName ? (
            <span className="ml-2 font-normal text-neutral-500">{course.courseShortName}</span>
          ) : null}{' '}
          <span className="font-normal text-neutral-500">{course.items.length}</span>
        </h3>
      </CollapsibleSummary>
      <ul className="flex flex-col gap-2">
        {course.items.map((item) => (
          <MarkRow key={item.id} item={item} onOpen={() => onOpen(platformEntry(item))} />
        ))}
      </ul>
    </details>
  );
}

/** Every graded item of the tracked courses, grouped by university and course. */
export default function MarksPage() {
  const navigate = useNavigate();
  const { data, error } = useQuery({
    queryKey: queryKeys.tracker(MARKS_QUERY),
    queryFn: () => api.getTracker(MARKS_QUERY),
  });

  const entries = useMemo<TrackerEntry[] | undefined>(() => data?.items.map(platformEntry), [data]);
  const groups = useMemo(() => (data ? groupMarks(data) : []), [data]);
  const { openEntry, open, close } = useOpenEntry(entries, false);

  if (error && !data) return <ErrorMessage error={toCommandError(error)} />;
  if (!data) return <p className={TEXT_MUTED}>Loading…</p>;

  return (
    <div className="flex flex-col gap-6">
      <h1 className={PAGE_TITLE}>Marks</h1>
      {data.universities.length === 0 ? (
        <div className="flex flex-col items-start gap-2">
          <p className={TEXT_MUTED}>No marks yet.</p>
          <button type="button" onClick={() => void navigate('/universities')} className={PRIMARY_BUTTON}>
            Add a university
          </button>
        </div>
      ) : groups.length === 0 ? (
        <p className={TEXT_MUTED}>No marks yet. Graded work appears here after a sync.</p>
      ) : (
        groups.map((university) => (
          <section key={university.universityId} className="flex flex-col gap-3">
            <h2 className={SECTION_TITLE}>{university.universityName}</h2>
            {university.courses.map((course) => (
              <CourseSection key={course.courseId} course={course} onOpen={open} />
            ))}
          </section>
        ))
      )}
      <EntryDetailsModal entry={openEntry} onClose={close} />
    </div>
  );
}
