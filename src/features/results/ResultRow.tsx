import type { ResultDto } from '../../lib/api/types';
import CategoryLabel from '../../components/results/CategoryLabel';
import PriorityBadge from '../../components/results/PriorityBadge';
import ResultDates from '../../components/results/ResultDates';
import ResultActions from '../../components/results/ResultActions';
import { formatDate, formatDateTime, formatResultDate } from '../../components/results/format';

export interface ResultRowProps {
  result: ResultDto;
  expanded: boolean;
  onToggle: () => void;
  onSetSeen: (id: number, seen: boolean) => void;
}

/** The nearest date at or after today, or the earliest date when all are in the past; "—" when there are none. */
function nextDateLabel(result: ResultDto): string {
  if (result.dates.length === 0) return '—';
  const today = new Date().toISOString().slice(0, 10);
  const byDate = (a: { date: string; time: string | null }, b: { date: string; time: string | null }) =>
    a.date.localeCompare(b.date) || (a.time ?? '').localeCompare(b.time ?? '');
  const upcoming = result.dates.filter((d) => d.date >= today).sort(byDate)[0];
  const chosen = upcoming ?? [...result.dates].sort(byDate)[0];
  return formatResultDate(chosen.date, chosen.time);
}

/**
 * A results-table row: unseen dot, subject + sender/course/received date,
 * category, next date, priority. Clicking the row expands inline details
 * (sender address, account, received/detected dates, all dates, flags) with
 * "Open email"/"Copy Message-ID" and "Mark as seen"/"Mark as new".
 */
export default function ResultRow({ result, expanded, onToggle, onSetSeen }: ResultRowProps) {
  return (
    <>
      <tr onClick={onToggle} aria-expanded={expanded} className="cursor-pointer border-b border-neutral-100 hover:bg-neutral-50">
        <td className="w-6 py-2">
          {!result.seen ? <span aria-label="Unseen" className="block h-2 w-2 rounded-full bg-blue-600" /> : null}
        </td>
        <td className="py-2 pr-4">
          <p className="font-medium text-neutral-900">{result.subject}</p>
          <p className="text-xs text-neutral-500">
            {result.senderName ?? result.senderEmail}
            {result.courseName ? ` · ${result.courseName}` : ''} · received {formatDate(result.receivedAt)}
          </p>
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
                </p>
              ) : null}
              <div className="flex items-center gap-4 pt-1">
                <ResultActions result={result} />
                <button
                  type="button"
                  onClick={() => onSetSeen(result.id, !result.seen)}
                  className="font-medium text-neutral-900 hover:underline"
                >
                  {result.seen ? 'Mark as new' : 'Mark as seen'}
                </button>
              </div>
            </div>
          </td>
        </tr>
      ) : null}
    </>
  );
}
