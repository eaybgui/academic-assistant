import type { Counters, ErrorKind, RunDto } from '../../lib/api/types';
import { formatDateTime, formatDuration } from '../../components/results/format';

const STATUS_LABELS: Record<RunDto['status'], string> = {
  running: 'Running',
  success: 'Success',
  partial: 'Partial',
  failed: 'Failed',
};

const STATUS_STYLES: Record<RunDto['status'], string> = {
  running: 'bg-blue-100 text-blue-700',
  success: 'bg-green-100 text-green-700',
  partial: 'bg-amber-100 text-amber-800',
  failed: 'bg-red-100 text-red-700',
};

const ERROR_KIND_LABELS: Record<ErrorKind, string> = {
  auth: 'Authentication error',
  connection: 'Connection error',
  protocol: 'Protocol error',
  ai: 'AI error',
  internal: 'Internal error',
};

const TRIGGER_LABELS: Record<RunDto['trigger'], string> = {
  manual: 'Manual',
};

function formatCounters(counters: Counters): string {
  return `found ${counters.found} · filtered ${counters.filtered} · analyzed ${counters.analyzed} · failed ${counters.failed} · results ${counters.resultsCreated}`;
}

export interface RunRowProps {
  run: RunDto;
  expanded: boolean;
  onToggle: () => void;
}

/** One run in the history list: status, start time, duration, trigger, counters; expands to per-account detail. */
export default function RunRow({ run, expanded, onToggle }: RunRowProps) {
  const duration = formatDuration(run.startedAt, run.finishedAt);

  return (
    <li className="rounded-lg border border-neutral-200 bg-white">
      <button
        type="button"
        onClick={onToggle}
        aria-expanded={expanded}
        className="flex w-full flex-wrap items-center justify-between gap-x-4 gap-y-1 px-4 py-3 text-left text-sm"
      >
        <div className="flex flex-wrap items-center gap-x-3 gap-y-1">
          <span
            className={`inline-flex items-center rounded-full px-2 py-0.5 text-xs font-medium ${STATUS_STYLES[run.status]}`}
          >
            {STATUS_LABELS[run.status]}
          </span>
          <span className="text-neutral-700">{formatDateTime(run.startedAt)}</span>
          {duration ? <span className="text-neutral-500">· {duration}</span> : null}
          <span className="text-neutral-500">· {TRIGGER_LABELS[run.trigger]}</span>
        </div>
        <span className="text-neutral-500">{formatCounters(run.counters)}</span>
      </button>
      {expanded ? (
        <div className="space-y-2 border-t border-neutral-100 px-4 py-3 text-sm">
          {run.errorSummary ? <p className="text-red-700">{run.errorSummary}</p> : null}
          {run.accounts.length === 0 ? (
            <p className="text-neutral-500">No accounts processed.</p>
          ) : (
            run.accounts.map((account) => (
              <div key={account.accountId} className="rounded border border-neutral-100 px-3 py-2">
                <p className="font-medium text-neutral-900">
                  {account.accountName} — {account.status === 'success' ? 'Success' : 'Failed'}
                </p>
                {account.status === 'failed' ? (
                  <p className="text-neutral-600">
                    {account.errorKind ? `${ERROR_KIND_LABELS[account.errorKind]}: ` : ''}
                    {account.errorMessage}
                  </p>
                ) : null}
                <p className="text-xs text-neutral-500">{formatCounters(account.counters)}</p>
              </div>
            ))
          )}
        </div>
      ) : null}
    </li>
  );
}
