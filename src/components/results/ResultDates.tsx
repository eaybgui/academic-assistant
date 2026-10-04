import type { ResultDateDto } from '../../lib/api/types';
import { DATE_KIND_LABELS } from './labels';
import { formatResultDate } from './format';

export interface ResultDatesProps {
  dates: ResultDateDto[];
}

/** Renders one chip per date, e.g. "Exam: 27 Oct 10:00". Renders nothing for an empty list. */
export default function ResultDates({ dates }: ResultDatesProps) {
  return (
    <>
      {dates.map((d, index) => (
        <span
          key={`${d.date}-${d.time ?? ''}-${d.kind}-${index}`}
          className="inline-flex items-center rounded-full bg-neutral-100 px-2 py-0.5 text-xs font-medium text-neutral-700"
        >
          {DATE_KIND_LABELS[d.kind]}: {formatResultDate(d.date, d.time)}
        </span>
      ))}
    </>
  );
}
