import { useEffect, useState } from 'react';
import { useQuery } from '@tanstack/react-query';
import { useLocation, useNavigate } from 'react-router';
import ErrorMessage from '../../components/ErrorMessage';
import { api, toCommandError } from '../../lib/api/client';
import { queryKeys } from '../../lib/api/queryKeys';
import AddUniversityModal from './AddUniversityModal';
import { useSync } from '../sync/useSync';
import UniversityCard from './UniversityCard';

/** Universities screen: LMS platforms with their sign-in state, plus the add flow. */
export default function UniversitiesPage() {
  const universitiesQuery = useQuery({ queryKey: queryKeys.universities, queryFn: api.getUniversities });

  // Other screens link here with `state: { openAdd: true }` to start the add wizard right away.
  const location = useLocation();
  const navigate = useNavigate();
  const openAddRequested = (location.state as { openAdd?: boolean } | null)?.openAdd === true;
  const [addOpen, setAddOpen] = useState(openAddRequested);
  // Consume the request, so going back to this entry doesn't reopen the wizard.
  useEffect(() => {
    if (openAddRequested) void navigate(location.pathname, { replace: true, state: null });
  }, [openAddRequested, navigate, location.pathname]);
  const { error: syncError } = useSync();
  // Part of the modal's `key`, so each open starts with a fresh form.
  const [addOpenCount, setAddOpenCount] = useState(0);

  function openAdd() {
    setAddOpenCount((count) => count + 1);
    setAddOpen(true);
  }

  const addButton = (
    <button
      type="button"
      onClick={openAdd}
      className="rounded bg-neutral-900 px-3 py-1.5 text-sm font-medium text-white hover:bg-neutral-800"
    >
      Add university
    </button>
  );

  const universities = universitiesQuery.data;
  const isEmpty = universities?.length === 0;

  return (
    <div className="flex flex-col gap-4">
      <div className="flex items-center justify-between">
        <h1 className="text-2xl font-semibold text-neutral-900">Universities</h1>
        {isEmpty ? null : addButton}
      </div>

      <ErrorMessage error={syncError} />
      <ErrorMessage error={universitiesQuery.error ? toCommandError(universitiesQuery.error) : null} />

      {universitiesQuery.isLoading ? <p className="text-sm text-neutral-500">Loading…</p> : null}
      {isEmpty ? (
        <div className="flex flex-col items-start gap-3">
          <p className="text-sm text-neutral-500">No universities yet</p>
          {addButton}
        </div>
      ) : null}

      <ul className="flex flex-col gap-2">
        {(universities ?? []).map((university) => (
          <UniversityCard key={university.id} university={university} />
        ))}
      </ul>

      <AddUniversityModal key={addOpenCount} open={addOpen} onClose={() => setAddOpen(false)} />
    </div>
  );
}
