import { useEffect, useRef, useState, type ReactNode } from 'react';
import { useQueryClient } from '@tanstack/react-query';
import { api, toCommandError } from '../../lib/api/client';
import { onRunFinished, onRunProgress } from '../../lib/api/events';
import type { CommandError, DashboardDto, RunProgressEvent, RunStage } from '../../lib/api/types';
import { CheckRunContext, type CheckRunState } from './useCheckRun';

const STAGE_LABELS: Record<RunStage, string> = {
  connecting: 'Connecting',
  fetching: 'Fetching',
  analyzing: 'Analyzing',
  done: 'Done',
  failed: 'Failed',
};

function formatProgress(event: RunProgressEvent): string {
  const counts = event.stage === 'analyzing' && event.total > 0 ? ` ${event.processed}/${event.total}` : '';
  return `${event.accountName}: ${STAGE_LABELS[event.stage]}${counts}`;
}

export interface CheckRunProviderProps {
  children: ReactNode;
}

/**
 * App-level "Check now" state machine, mounted once (in `main.tsx`, inside the
 * `QueryClientProvider`) so `run:progress`/`run:finished` are subscribed to for the whole app
 * lifetime. Because the subscription is not deferred until `triggerCheck()` resolves, a run that
 * finishes very quickly - even before the `checkNow` call returns its run id - is never missed.
 * `run:finished` invalidates the dashboard/results/runs query prefixes unconditionally, so every
 * route reacts to a finished run, not just the dashboard.
 */
export function CheckRunProvider({ children }: CheckRunProviderProps) {
  const queryClient = useQueryClient();
  const [runningRunId, setRunningRunId] = useState<number | null>(null);
  // The latest progress event seen for any run; only shown while it belongs to the running run.
  const [progressEvent, setProgressEvent] = useState<RunProgressEvent | null>(null);
  const [error, setError] = useState<CommandError | null>(null);

  // Every run id we've seen `run:finished` for, so a `checkNow()` call that resolves after its
  // run already finished doesn't re-adopt it as still running.
  const finishedRunIdsRef = useRef<Set<number>>(new Set());

  useEffect(() => {
    let unlistenProgress: (() => void) | undefined;
    let unlistenFinished: (() => void) | undefined;
    let cancelled = false;

    onRunProgress(setProgressEvent).then((unlisten) => {
      if (cancelled) unlisten();
      else unlistenProgress = unlisten;
    });

    onRunFinished((event) => {
      finishedRunIdsRef.current.add(event.runId);
      setRunningRunId((current) => (current === event.runId ? null : current));
      setProgressEvent((current) => (current?.runId === event.runId ? null : current));
      void queryClient.invalidateQueries({ queryKey: ['dashboard'] });
      void queryClient.invalidateQueries({ queryKey: ['results'] });
      void queryClient.invalidateQueries({ queryKey: ['runs'] });
    }).then((unlisten) => {
      if (cancelled) unlisten();
      else unlistenFinished = unlisten;
    });

    return () => {
      cancelled = true;
      unlistenProgress?.();
      unlistenFinished?.();
    };
  }, [queryClient]);

  // Reconciles with the dashboard's own view of run state: adopts a run the backend reports as
  // running when nothing is being tracked locally yet (e.g. a check already in progress when the
  // app started), and clears a locally-tracked run once the dashboard confirms it has finished
  // (a fallback for a `run:finished` event that arrived before this provider was listening).
  useEffect(() => {
    const unsubscribe = queryClient.getQueryCache().subscribe((event) => {
      if (event.type !== 'updated' && event.type !== 'added') return;
      if (event.query.queryKey[0] !== 'dashboard') return;
      const data = event.query.state.data as DashboardDto | undefined;
      if (!data) return;

      setRunningRunId((current) => {
        if (current !== null && data.runningRunId === null && data.lastRun !== null && data.lastRun.id >= current) {
          return null;
        }
        if (current === null && data.runningRunId !== null && !finishedRunIdsRef.current.has(data.runningRunId)) {
          return data.runningRunId;
        }
        return current;
      });
    });
    return unsubscribe;
  }, [queryClient]);

  async function triggerCheck() {
    setError(null);
    try {
      const runId = await api.checkNow();
      if (!finishedRunIdsRef.current.has(runId)) {
        setProgressEvent(null);
        setRunningRunId(runId);
      }
    } catch (e) {
      setError(toCommandError(e));
    }
  }

  const value: CheckRunState = {
    running: runningRunId !== null,
    progress: progressEvent && progressEvent.runId === runningRunId ? formatProgress(progressEvent) : null,
    error,
    triggerCheck,
  };

  return <CheckRunContext.Provider value={value}>{children}</CheckRunContext.Provider>;
}
