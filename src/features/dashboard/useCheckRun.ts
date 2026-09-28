import { useEffect, useState } from 'react';
import { useQueryClient } from '@tanstack/react-query';
import { api, toCommandError } from '../../lib/api/client';
import { onRunFinished, onRunProgress } from '../../lib/api/events';
import type { CommandError, RunProgressEvent, RunStage } from '../../lib/api/types';

const STAGE_LABELS: Record<RunStage, string> = {
  connecting: 'connecting',
  fetching: 'fetching',
  analyzing: 'analyzing',
  done: 'done',
  failed: 'failed',
};

function formatProgress(event: RunProgressEvent): string {
  const counts = event.stage === 'analyzing' && event.total > 0 ? ` ${event.processed}/${event.total}` : '';
  return `${event.accountName}: ${STAGE_LABELS[event.stage]}${counts}`;
}

export interface UseCheckRunResult {
  isRunning: boolean;
  progressLabel: string | null;
  triggerCheck: () => Promise<void>;
  checkError: CommandError | null;
}

/**
 * Tracks the currently running check (if any): starts one via `checkNow`,
 * follows `run:progress` / `run:finished` for the active run id (whether it
 * was started here or was already running when the dashboard loaded), and
 * invalidates the dashboard/results/runs queries once it finishes.
 */
export function useCheckRun(dashboardRunningRunId: number | null): UseCheckRunResult {
  const queryClient = useQueryClient();
  // A run started from this hook (via `triggerCheck`), overriding the
  // dashboard's `runningRunId` until it finishes.
  const [startedRunId, setStartedRunId] = useState<number | null>(null);
  // The last run id we saw `run:finished` for, so a dashboard fetch that is
  // still stale (hasn't picked up the finish yet) doesn't re-show it as running.
  const [finishedRunId, setFinishedRunId] = useState<number | null>(null);
  const [progress, setProgress] = useState<RunProgressEvent | null>(null);
  const [checkError, setCheckError] = useState<CommandError | null>(null);

  // Adopts a run already in progress on the backend (e.g. dashboard loaded
  // while a check was running) without an effect: it's a plain derived value.
  const runningRunId =
    startedRunId ??
    (dashboardRunningRunId !== null && dashboardRunningRunId !== finishedRunId ? dashboardRunningRunId : null);

  useEffect(() => {
    if (runningRunId === null) return undefined;

    let unlistenProgress: (() => void) | undefined;
    let unlistenFinished: (() => void) | undefined;
    let cancelled = false;

    onRunProgress((event) => {
      if (event.runId === runningRunId) setProgress(event);
    }).then((unlisten) => {
      if (cancelled) unlisten();
      else unlistenProgress = unlisten;
    });

    onRunFinished((event) => {
      if (event.runId !== runningRunId) return;
      setStartedRunId(null);
      setFinishedRunId(runningRunId);
      setProgress(null);
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
  }, [runningRunId, queryClient]);

  async function triggerCheck() {
    setCheckError(null);
    try {
      const runId = await api.checkNow();
      setFinishedRunId(null);
      setStartedRunId(runId);
    } catch (e) {
      setCheckError(toCommandError(e));
    }
  }

  return {
    isRunning: runningRunId !== null,
    progressLabel: progress ? formatProgress(progress) : null,
    triggerCheck,
    checkError,
  };
}
