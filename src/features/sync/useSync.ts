import { createContext, useContext } from 'react';
import type { CommandError, LmsSyncResultDto } from '../../lib/api/types';

export interface SyncState {
  /** True from a successful `startSync` until `lms-sync:finished`. */
  running: boolean;
  /** Human label of the latest progress event while running, e.g. "UV: Fetching items". */
  progress: string | null;
  /** Results of the last finished sync (empty array after an interrupted sync); null before any. */
  lastResults: LmsSyncResultDto[] | null;
  /** Error of the last `startSync` call (e.g. conflict "A sync is already running"). */
  error: CommandError | null;
  startSync: (universityId: number | null) => Promise<void>;
  clearResults: () => void;
}

export const SyncContext = createContext<SyncState | null>(null);

/** Reads the app-level LMS sync state. Must be used within a `SyncProvider`. */
export function useSync(): SyncState {
  const ctx = useContext(SyncContext);
  if (!ctx) throw new Error('useSync must be used within a SyncProvider');
  return ctx;
}
