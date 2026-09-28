import type { ResultDto } from '../../lib/api/types';
import PriorityBadge from './PriorityBadge';
import CategoryLabel from './CategoryLabel';
import ResultDates from './ResultDates';
import ResultActions from './ResultActions';
import { formatDate } from './format';

export interface ResultCardProps {
  result: ResultDto;
  onSetSeen: (id: number, seen: boolean) => void;
}

/**
 * A result card for the dashboard's "New results" list: subject, priority badge,
 * meta line ("sender · course (university) · account · received <date>"),
 * category/date/flag chips, and the "Open email"/"Copy Message-ID" + "Seen" actions.
 */
export default function ResultCard({ result, onSetSeen }: ResultCardProps) {
  const senderLabel = result.senderName ?? result.senderEmail;
  const courseLabel = result.courseName
    ? result.universityName
      ? `${result.courseName} (${result.universityName})`
      : result.courseName
    : result.universityName;

  const metaParts = [senderLabel, courseLabel, result.accountName, `received ${formatDate(result.receivedAt)}`].filter(
    (part): part is string => Boolean(part),
  );

  return (
    <article className="rounded-lg border border-neutral-200 bg-white p-4">
      <div className="flex items-start justify-between gap-3">
        <div className="min-w-0">
          <div className="flex items-center gap-2">
            <h3 className="truncate text-sm font-semibold text-neutral-900">{result.subject}</h3>
            <PriorityBadge priority={result.priority} />
          </div>
          <p className="mt-0.5 text-xs text-neutral-500">{metaParts.join(' · ')}</p>
        </div>
      </div>

      <div className="mt-2 flex flex-wrap gap-1.5">
        <CategoryLabel category={result.category} />
        <ResultDates dates={result.dates} />
        {result.actionRequired ? (
          <span className="inline-flex items-center rounded-full bg-red-50 px-2 py-0.5 text-xs font-medium text-red-700">
            Action required
          </span>
        ) : null}
        {result.possibleChange ? (
          <span className="inline-flex items-center rounded-full bg-amber-50 px-2 py-0.5 text-xs font-medium text-amber-700">
            Possible change
          </span>
        ) : null}
      </div>

      <div className="mt-3 flex justify-end gap-4 text-sm">
        <ResultActions result={result} />
        <button
          type="button"
          onClick={() => onSetSeen(result.id, true)}
          className="font-medium text-neutral-900 hover:underline"
        >
          Seen
        </button>
      </div>
    </article>
  );
}
