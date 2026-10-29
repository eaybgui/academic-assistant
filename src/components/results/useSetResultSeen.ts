import { useMutation, useQueryClient } from '@tanstack/react-query';
import { api, toCommandError } from '../../lib/api/client';
import type { CommandError } from '../../lib/api/types';

export interface UseSetResultSeenResult {
  /** Marks a result seen/unseen. */
  setSeen: (id: number, seen: boolean) => void;
  /** Whether the mutation for this particular result id is in flight. */
  isPending: (id: number) => boolean;
  /** The error from the most recent failed call, if any (cleared automatically on retry). */
  error: CommandError | null;
  /** The result id the current `error` belongs to, or `null` when there is no error. */
  erroredId: number | null;
}

/**
 * "Set result seen" mutation for the announcements results table rows:
 * calls `setResultSeen`, invalidates the dashboard/results query prefixes on success, and exposes
 * per-row pending/error state so a failure shows up next to the row that triggered it.
 */
export function useSetResultSeen(): UseSetResultSeenResult {
  const queryClient = useQueryClient();

  const mutation = useMutation({
    mutationFn: ({ id, seen }: { id: number; seen: boolean }) => api.setResultSeen(id, seen),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: ['dashboard'] });
      void queryClient.invalidateQueries({ queryKey: ['results'] });
    },
  });

  return {
    setSeen: (id, seen) => mutation.mutate({ id, seen }),
    isPending: (id) => mutation.isPending && mutation.variables?.id === id,
    error: mutation.isError ? toCommandError(mutation.error) : null,
    erroredId: mutation.isError ? (mutation.variables?.id ?? null) : null,
  };
}
