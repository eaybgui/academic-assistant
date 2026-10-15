import { useEffect, useRef, useState, type ReactNode } from 'react';
import { useQueryClient } from '@tanstack/react-query';
import { api, toCommandError } from '../../lib/api/client';
import { onLmsSyncFinished, onLmsSyncProgress } from '../../lib/api/events';
import type { CommandError, LmsSyncProgressEvent, LmsSyncResultDto, LmsSyncStage } from '../../lib/api/types';
import { SyncContext, type SyncState } from './useSync';

const STAGE_LABELS: Record<LmsSyncStage, string> = {
  courses: 'Checking courses',
  items: 'Fetching items',
  saving: 'Saving',
  done: 'Done',
  failed: 'Failed',
  login_required: 'Sign-in required',
};

function formatProgress(event: LmsSyncProgressEvent): string {
  return `${event.universityName}: ${STAGE_LABELS[event.stage]}`;
}

export interface SyncProviderProps {
  children: ReactNode;
}

/**
 * App-level LMS sync state machine, mounted once (in `main.tsx`, inside the `QueryClientProvider`)
 * so `lms-sync:progress`/`lms-sync:finished` are subscribed to for the whole app lifetime. A sync
 * that finishes very quickly - even before the `lmsSync` call returns - is never missed, and a sync
 * started from another view (or already in progress) is reflected as running as soon as its first
 * progress event arrives. `lms-sync:finished` invalidates the tracker/universities/courses query
 * prefixes unconditionally, so every route reacts to a finished sync.
 */
export function SyncProvider({ children }: SyncProviderProps) {
  const queryClient = useQueryClient();
  const [running, setRunning] = useState(false);
  const [progress, setProgress] = useState<string | null>(null);
  const [lastResults, setLastResults] = useState<LmsSyncResultDto[] | null>(null);
  const [error, setError] = useState<CommandError | null>(null);

  // Counts `lms-sync:finished` events, so a `lmsSync()` call that resolves after its sync already
  // finished doesn't leave the UI stuck as running.
  const finishedCountRef = useRef(0);

  useEffect(() => {
    let unlistenProgress: (() => void) | undefined;
    let unlistenFinished: (() => void) | undefined;
    let cancelled = false;

    onLmsSyncProgress((event) => {
      setRunning(true);
      setProgress(formatProgress(event));
    }).then((unlisten) => {
      if (cancelled) unlisten();
      else unlistenProgress = unlisten;
    });

    onLmsSyncFinished((event) => {
      finishedCountRef.current += 1;
      setRunning(false);
      setProgress(null);
      setError(null);
      setLastResults(event.results);
      void queryClient.invalidateQueries({ queryKey: ['tracker'] });
      void queryClient.invalidateQueries({ queryKey: ['universities'] });
      void queryClient.invalidateQueries({ queryKey: ['courses'] });
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

  async function startSync(universityId: number | null) {
    setError(null);
    const finishedBefore = finishedCountRef.current;
    try {
      await api.lmsSync(universityId);
      if (finishedCountRef.current === finishedBefore) setRunning(true);
    } catch (e) {
      setError(toCommandError(e));
    }
  }

  const value: SyncState = {
    running,
    progress: running ? progress : null,
    lastResults,
    error,
    startSync,
    clearResults: () => setLastResults(null),
  };

  return <SyncContext.Provider value={value}>{children}</SyncContext.Provider>;
}
