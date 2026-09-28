import type { ResultSort } from '../../lib/api/types';
import { RESULT_SORT_LABELS } from './labels';

const OPTIONS: ResultSort[] = ['upcoming', 'priority', 'received'];

export interface SortSelectProps {
  value: ResultSort;
  onChange: (sort: ResultSort) => void;
  id?: string;
}

/** Sort selector for results: nearest upcoming date (default), priority, received date. */
export default function SortSelect({ value, onChange, id }: SortSelectProps) {
  return (
    <select
      id={id}
      aria-label="Sort"
      value={value}
      onChange={(e) => onChange(e.target.value as ResultSort)}
      className="rounded border border-neutral-300 bg-white px-2 py-1.5 text-sm text-neutral-700"
    >
      {OPTIONS.map((option) => (
        <option key={option} value={option}>
          {RESULT_SORT_LABELS[option]}
        </option>
      ))}
    </select>
  );
}
