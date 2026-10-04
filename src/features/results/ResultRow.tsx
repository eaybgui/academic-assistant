import type { CommandError, ResultDto } from '../../lib/api/types';
import CategoryLabel from '../../components/results/CategoryLabel';
import PriorityBadge from '../../components/results/PriorityBadge';
import ResultDates from '../../components/results/ResultDates';
import ResultActions from '../../components/results/ResultActions';
import ResultFlags from '../../components/results/ResultFlags';
import ErrorMessage from '../../components/ErrorMessage';
import { formatDate, formatDateTime, formatResultDate, todayLocalIso } from '../../components/results/format';

export interface ResultRowProps {
  result: ResultDto;
  expanded: boolean;
  onToggle: () => void;
  onSetSeen: (id: number, seen: boolean) => void;
  pending?: boolean;
  error?: CommandError | null;
}

/** The nearest date at or after today, or the earliest date when all are in the past; "—" when there are none. */
function nextDateLabel(result: ResultDto): string {
  if (result.dates.length === 0) return '—';
  const today = todayLocalIso();
  const byDate = (a: { date: string; time: string | null }, b: { date: string; time: string | null }) =>
    a.date.localeCompare(b.date) || (a.time ?? '').localeCompare(b.time ?? '');
  const upcoming = result.dates.filter((d) => d.date >= today).sort(byDate)[0];
  const chosen = upcoming ?? [...result.dates].sort(byDate)[0];
  return formatResultDate(chosen.date, chosen.time);
}

/**
 * A results-table row: unseen dot, subject + sender/course/received date,
 * category, next date, priority. The subject cell is a button (keyboard-accessible) that
 * toggles inline details (sender address, account, received/detected dates, all dates, flags)
 * with "Open email"/"Copy Message-ID" and "Mark as seen"/"Mark as new".
 */
export default function ResultRow({
  result,
  expanded,
  onToggle,
  onSetSeen,
  pending = false,
  error = null,
}: ResultRowProps) {
  return (
    <>
      <tr className="border-b border-neutral-100 hover:bg-neutral-50">
        <td className="w-6 py-2">
          {!result.seen ? (
            <span className="block h-2 w-2 rounded-full bg-blue-600">
              <span className="sr-only">Unseen</span>
            </span>
          ) : null}
        </td>
        <td className="py-2 pr-4">
          <button type="button" onClick={onToggle} aria-expanded={expanded} className="w-full text-left">
            <p className="font-medium text-neutral-900">{result.subject}</p>
            <p className="text-xs text-neutral-500">
              {result.senderName ?? result.senderEmail}
              {result.courseName ? ` · ${result.courseName}` : ''} · received {formatDate(result.receivedAt)}
            </p>
          </button>
        </td>
        <td className="py-2 pr-4">
          <CategoryLabel category={result.category} />
        </td>
        <td className="py-2 pr-4 text-sm text-neutral-700">{nextDateLabel(result)}</td>
        <td className="py-2">
          <PriorityBadge priority={result.priority} />
        </td>
      </tr>
      {expanded ? (
        <tr className="border-b border-neutral-100 bg-neutral-50">
          <td colSpan={5} className="px-4 py-3">
            <div className="space-y-2 text-sm text-neutral-700">
              <p>
                <span className="font-medium">Sender:</span> {result.senderName ? `${result.senderName} ` : ''}
                &lt;{result.senderEmail}&gt;
              </p>
              <p>
                <span className="font-medium">Account:</span> {result.accountName}
              </p>
              <p>
                <span className="font-medium">Received:</span> {formatDateTime(result.receivedAt)}
              </p>
              <p>
                <span className="font-medium">Detected:</span> {formatDateTime(result.detectedAt)}
              </p>
              {result.dates.length > 0 ? (
                <div className="flex flex-wrap gap-1.5">
                  <ResultDates dates={result.dates} />
                </div>
              ) : null}
              {result.actionRequired || result.possibleChange ? (
                <p className="space-x-2">
                  <ResultFlags actionRequired={result.actionRequired} possibleChange={result.possibleChange} />
                </p>
              ) : null}
              <div className="flex items-center gap-4 pt-1">
                <ResultActions result={result} />
                <button
                  type="button"
                  disabled={pending}
                  onClick={() => onSetSeen(result.id, !result.seen)}
                  className="font-medium text-neutral-900 hover:underline disabled:opacity-50"
                >
                  {result.seen ? 'Mark as new' : 'Mark as seen'}
                </button>
                <ErrorMessage error={error} />
              </div>
            </div>
          </td>
        </tr>
      ) : null}
    </>
  );
}
