import type { CommandError, ResultDto } from '../../lib/api/types';
import PriorityBadge from './PriorityBadge';
import CategoryLabel from './CategoryLabel';
import ResultDates from './ResultDates';
import ResultActions from './ResultActions';
import ResultFlags from './ResultFlags';
import ErrorMessage from '../ErrorMessage';
import { formatDate } from './format';

export interface ResultCardProps {
  result: ResultDto;
  onSetSeen: (id: number, seen: boolean) => void;
  pending?: boolean;
  error?: CommandError | null;
}

/**
 * A result card for the dashboard's "New results" list: subject, priority badge,
 * meta line ("sender · course (university) · account · received <date>"),
 * category/date/flag chips, and the "Open email"/"Copy Message-ID" + "Seen" actions.
 */
export default function ResultCard({ result, onSetSeen, pending = false, error = null }: ResultCardProps) {
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
        <ResultFlags actionRequired={result.actionRequired} possibleChange={result.possibleChange} />
      </div>

      <div className="mt-3 flex items-center justify-end gap-4 text-sm">
        <ErrorMessage error={error} />
        <ResultActions result={result} />
        <button
          type="button"
          disabled={pending}
          onClick={() => onSetSeen(result.id, true)}
          className="font-medium text-neutral-900 hover:underline disabled:opacity-50"
        >
          Seen
        </button>
      </div>
    </article>
  );
}
