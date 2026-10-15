import { useState } from 'react';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import ErrorMessage from '../../components/ErrorMessage';
import { api, toCommandError } from '../../lib/api/client';
import type { CommandError, LmsSyncResultDto, LmsUniversityDto } from '../../lib/api/types';
import { useSync } from '../sync/useSync';

const BANNER = 'flex flex-col gap-2 rounded-lg border px-4 py-3 text-sm';
const ACTION_BUTTON =
  'rounded border border-neutral-300 bg-white px-3 py-1.5 text-sm text-neutral-700 hover:bg-neutral-50 disabled:opacity-50';

function needsSignIn(university: LmsUniversityDto): boolean {
  return (
    university.loginState === 'expired' ||
    (university.lastSync.outcome === 'login_required' && university.loginState !== 'ok')
  );
}

function SignInBanner({ university }: { university: LmsUniversityDto }) {
  const { startSync } = useSync();
  const queryClient = useQueryClient();
  const [pending, setPending] = useState(false);
  const [error, setError] = useState<CommandError | null>(null);

  async function handleSignIn() {
    setPending(true);
    setError(null);
    try {
      await api.lmsSignIn(university.id);
      void queryClient.invalidateQueries({ queryKey: ['tracker'] });
      await startSync(university.id);
    } catch (e) {
      setError(toCommandError(e));
    } finally {
      setPending(false);
    }
  }

  return (
    <div role="alert" className={`${BANNER} border-amber-300 bg-amber-50 text-amber-900`}>
      <div className="flex flex-wrap items-center justify-between gap-2">
        <span>{university.name} needs you to sign in again</span>
        <button
          type="button"
          disabled={pending}
          onClick={() => void handleSignIn()}
          className={ACTION_BUTTON}
        >
          Sign in
        </button>
      </div>
      {error ? <p className="text-red-600">{error.message}</p> : null}
    </div>
  );
}

function SyncSummaryBanner() {
  const { lastResults, clearResults } = useSync();
  const queryClient = useQueryClient();
  const dismissAll = useMutation({
    mutationFn: () => api.dismissAllChanges(null),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: ['tracker'] });
    },
  });

  if (lastResults === null) return null;
  if (lastResults.length === 0) {
    return (
      <div role="alert" className={`${BANNER} border-neutral-200 bg-white text-neutral-700`}>
        <div className="flex flex-wrap items-center justify-between gap-2">
          <span>The sync was interrupted. Try again.</span>
          <button type="button" onClick={clearResults} className={ACTION_BUTTON}>
            Close
          </button>
        </div>
      </div>
    );
  }

  const total = (pick: (r: LmsSyncResultDto) => number) =>
    lastResults.reduce((sum, r) => sum + pick(r), 0);
  const failed = lastResults.filter((r) => r.outcome !== 'success');

  return (
    <div role="alert" className={`${BANNER} border-neutral-200 bg-white text-neutral-700`}>
      <p>
        Sync finished: {total((r) => r.newItems)} new, {total((r) => r.dueChanged)} date changed,{' '}
        {total((r) => r.removed)} removed
      </p>
      {failed.map((r) => (
        <p key={r.universityId} className="text-red-600">
          {r.universityName}: {r.error ?? 'Sync failed'}
        </p>
      ))}
      <ErrorMessage error={dismissAll.isError ? toCommandError(dismissAll.error) : null} />
      <div className="flex gap-2">
        <button
          type="button"
          disabled={dismissAll.isPending}
          onClick={() => dismissAll.mutate()}
          className={ACTION_BUTTON}
        >
          Dismiss all changes
        </button>
        <button type="button" onClick={clearResults} className={ACTION_BUTTON}>
          Close
        </button>
      </div>
    </div>
  );
}

export interface SyncBannersProps {
  /** The universities currently shown (all of them, or the selected tab's). */
  universities: LmsUniversityDto[];
}

/** Sync feedback above the item list: sign-in-again prompts and the summary of the last sync. */
export default function SyncBanners({ universities }: SyncBannersProps) {
  return (
    <>
      {universities.filter(needsSignIn).map((u) => (
        <SignInBanner key={u.id} university={u} />
      ))}
      <SyncSummaryBanner />
    </>
  );
}
