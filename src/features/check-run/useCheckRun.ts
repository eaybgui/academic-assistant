import { createContext, useContext } from 'react';
import type { CommandError } from '../../lib/api/types';

export interface CheckRunState {
  running: boolean;
  progress: string | null;
  error: CommandError | null;
  triggerCheck: () => Promise<void>;
}

export const CheckRunContext = createContext<CheckRunState | null>(null);

/** Reads the app-level "Check now" state. Must be used within a `CheckRunProvider`. */
export function useCheckRun(): CheckRunState {
  const ctx = useContext(CheckRunContext);
  if (!ctx) throw new Error('useCheckRun must be used within a CheckRunProvider');
  return ctx;
}
