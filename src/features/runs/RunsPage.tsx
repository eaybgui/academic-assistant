import { useState } from 'react';
import { useQuery } from '@tanstack/react-query';
import { api, toCommandError } from '../../lib/api/client';
import { queryKeys } from '../../lib/api/queryKeys';
import ErrorMessage from '../../components/ErrorMessage';
import RunRow from './RunRow';

const HISTORY_LIMIT = 50;

/** Run history: newest first, each expandable to its per-account results and errors. */
export default function RunsPage() {
  const [expandedId, setExpandedId] = useState<number | null>(null);

  const runsQuery = useQuery({
    queryKey: queryKeys.runs(HISTORY_LIMIT),
    queryFn: () => api.getRunHistory(HISTORY_LIMIT),
  });

  const runs = runsQuery.data ?? [];

  return (
    <div className="space-y-4">
      <h1 className="text-2xl font-semibold text-neutral-900">Runs</h1>

      {runsQuery.isError ? <ErrorMessage error={toCommandError(runsQuery.error)} /> : null}

      {runsQuery.isSuccess && runs.length === 0 ? (
        <p className="text-sm text-neutral-500">No checks have run yet.</p>
      ) : null}

      <ul className="space-y-2">
        {runs.map((run) => (
          <RunRow
            key={run.id}
            run={run}
            expanded={expandedId === run.id}
            onToggle={() => setExpandedId((id) => (id === run.id ? null : run.id))}
          />
        ))}
      </ul>
    </div>
  );
}
