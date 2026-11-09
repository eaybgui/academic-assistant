import type { ResultDateDto } from '../../lib/api/types';
import { DATE_KIND_LABELS } from './labels';
import { formatResultDate } from './format';
import { CHIP_NEUTRAL } from '../styles';

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
          className={CHIP_NEUTRAL}
        >
          {DATE_KIND_LABELS[d.kind]}: {formatResultDate(d.date, d.time)}
        </span>
      ))}
    </>
  );
}
