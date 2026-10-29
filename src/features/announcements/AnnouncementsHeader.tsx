import { useState } from 'react';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { api, toCommandError } from '../../lib/api/client';
import { queryKeys } from '../../lib/api/queryKeys';
import type { CommandError, RunDto } from '../../lib/api/types';
import ErrorMessage from '../../components/ErrorMessage';
import { formatRelativeDay, formatTime } from '../../components/results/format';
import { useCheckRun } from '../check-run/useCheckRun';

function formatLastCheck(run: RunDto | null): string {
  if (!run) return 'No checks yet.';
  const when = `${formatRelativeDay(run.startedAt)} ${formatTime(run.startedAt)}`;
  const status = run.errorSummary ? `${run.status}: ${run.errorSummary}` : run.status;
  return `Last check ${when} · ${status}`;
}

function MetricTile({ label, value }: { label: string; value: number }) {
  return (
    <div className="rounded-lg border border-neutral-200 bg-white px-4 py-3">
      <p className="text-2xl font-semibold text-neutral-900">{value}</p>
      <p className="text-xs text-neutral-500">{label}</p>
    </div>
  );
}

/** Title, last check, "Check now" with progress, attention banners, counters and "Mark all as seen". */
export default function AnnouncementsHeader() {
  const [isTriggering, setIsTriggering] = useState(false);
  const [markAllError, setMarkAllError] = useState<CommandError | null>(null);
  const queryClient = useQueryClient();

  const dashboardQuery = useQuery({
    queryKey: queryKeys.dashboard('upcoming'),
    queryFn: () => api.getDashboard('upcoming'),
  });

  const { running, progress, triggerCheck, error: checkError } = useCheckRun();

  async function handleCheckNow() {
    setIsTriggering(true);
    try {
      await triggerCheck();
    } finally {
      setIsTriggering(false);
    }
  }

  const markAllSeenMutation = useMutation({
    mutationFn: () => api.markAllSeen(),
    onSuccess: () => {
      setMarkAllError(null);
      void queryClient.invalidateQueries({ queryKey: ['dashboard'] });
      void queryClient.invalidateQueries({ queryKey: ['results'] });
    },
    onError: (e) => setMarkAllError(toCommandError(e)),
  });

  const dashboard = dashboardQuery.data;

  return (
    <div className="space-y-4">
      <div className="flex items-start justify-between">
        <div>
          <h1 className="text-2xl font-semibold text-neutral-900">Announcements</h1>
          <p className="mt-1 text-sm text-neutral-500">{formatLastCheck(dashboard?.lastRun ?? null)}</p>
        </div>
        <div className="text-right">
          {running ? (
            <p role="status" className="text-sm text-neutral-600">
              {progress ?? 'Checking…'}
            </p>
          ) : (
            <button
              type="button"
              disabled={isTriggering}
              onClick={() => void handleCheckNow()}
              className="rounded bg-neutral-900 px-3 py-1.5 text-sm font-medium text-white hover:bg-neutral-800 disabled:opacity-50"
            >
              Check now
            </button>
          )}
          <ErrorMessage error={checkError} />
        </div>
      </div>

      {dashboardQuery.isError ? <ErrorMessage error={toCommandError(dashboardQuery.error)} /> : null}

      {dashboard && dashboard.attention.length > 0 ? (
        <div className="space-y-2">
          {dashboard.attention.map((item) => (
            <div
              key={item.accountId}
              role="alert"
              className="rounded border border-amber-200 bg-amber-50 px-3 py-2 text-sm text-amber-800"
            >
              <span className="font-medium">{item.accountName}:</span> {item.message}
            </div>
          ))}
        </div>
      ) : null}

      <div className="grid grid-cols-5 gap-3">
        <MetricTile label="New" value={dashboard?.counts.newTotal ?? 0} />
        <MetricTile label="Tasks" value={dashboard?.counts.tasks ?? 0} />
        <MetricTile label="Exams" value={dashboard?.counts.exams ?? 0} />
        <MetricTile label="Events" value={dashboard?.counts.events ?? 0} />
        <MetricTile label="Changes and notices" value={dashboard?.counts.changesAndNotices ?? 0} />
      </div>

      <div className="flex items-center justify-end gap-3">
        <ErrorMessage error={markAllError} />
        <button
          type="button"
          disabled={markAllSeenMutation.isPending}
          onClick={() => markAllSeenMutation.mutate()}
          className="text-sm font-medium text-neutral-600 hover:text-neutral-900 disabled:opacity-50"
        >
          Mark all as seen
        </button>
      </div>
    </div>
  );
}
