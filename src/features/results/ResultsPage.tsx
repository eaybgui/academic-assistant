import { useEffect, useMemo, useState } from 'react';
import { keepPreviousData, useQuery } from '@tanstack/react-query';
import { api, toCommandError } from '../../lib/api/client';
import { queryKeys } from '../../lib/api/queryKeys';
import type { ResultQuery, ResultSort } from '../../lib/api/types';
import ErrorMessage from '../../components/ErrorMessage';
import { useSetResultSeen } from '../../components/results/useSetResultSeen';
import FiltersModal from './FiltersModal';
import { countActiveFilters, type ResultFilters } from './filterUtils';
import ResultRow from './ResultRow';

const PAGE_SIZE = 25;
const SEARCH_DEBOUNCE_MS = 300;

export default function ResultsPage() {
  const [searchInput, setSearchInput] = useState('');
  const [search, setSearch] = useState('');
  const [filters, setFilters] = useState<ResultFilters>({});
  const [sort, setSort] = useState<ResultSort>('upcoming');
  const [page, setPage] = useState(0);
  const [filtersOpen, setFiltersOpen] = useState(false);
  const [expandedId, setExpandedId] = useState<number | null>(null);
  const seen = useSetResultSeen();

  useEffect(() => {
    const timeout = setTimeout(() => {
      setSearch(searchInput.trim());
      setPage(0);
    }, SEARCH_DEBOUNCE_MS);
    return () => clearTimeout(timeout);
  }, [searchInput]);

  const query: ResultQuery = useMemo(
    () => ({
      search: search || undefined,
      seen: filters.seen,
      category: filters.category,
      accountId: filters.accountId,
      courseId: filters.courseId,
      sort,
      limit: PAGE_SIZE,
      offset: page * PAGE_SIZE,
    }),
    [search, filters, sort, page],
  );

  const resultsQuery = useQuery({
    queryKey: queryKeys.results(query),
    queryFn: () => api.getResults(query),
    placeholderData: keepPreviousData,
  });

  const accountsQuery = useQuery({ queryKey: queryKeys.accounts, queryFn: api.getAccounts });
  const catalogQuery = useQuery({ queryKey: queryKeys.catalog, queryFn: api.getCatalog });

  function handleApplyFilters(nextFilters: ResultFilters, nextSort: ResultSort) {
    setFilters(nextFilters);
    setSort(nextSort);
    setPage(0);
  }

  const items = resultsQuery.data?.items ?? [];
  const total = resultsQuery.data?.total ?? 0;
  const totalPages = Math.max(1, Math.ceil(total / PAGE_SIZE));
  const activeFilterCount = countActiveFilters(filters);

  // Clamps the current page once `total` shrinks below it (e.g. after a filter change or results
  // being marked seen out from under a "new only" filter), instead of leaving the page on an
  // empty page past the last one. Adjusting state during render is React's documented
  // alternative to an effect for this.
  if (page > totalPages - 1) {
    setPage(totalPages - 1);
  }

  return (
    <div className="space-y-4">
      <h1 className="text-2xl font-semibold text-neutral-900">Results</h1>

      <div className="flex items-center gap-3">
        <input
          type="search"
          value={searchInput}
          onChange={(e) => setSearchInput(e.target.value)}
          placeholder="Search subject or sender"
          aria-label="Search results"
          className="w-72 rounded border border-neutral-300 px-3 py-1.5 text-sm"
        />
        <button
          type="button"
          onClick={() => setFiltersOpen(true)}
          className="rounded border border-neutral-300 px-3 py-1.5 text-sm text-neutral-700 hover:bg-neutral-100"
        >
          Filters and sort
          {activeFilterCount > 0 ? (
            <span className="ml-2 inline-flex h-5 w-5 items-center justify-center rounded-full bg-neutral-900 text-xs font-medium text-white">
              {activeFilterCount}
            </span>
          ) : null}
        </button>
      </div>

      {resultsQuery.isError ? <ErrorMessage error={toCommandError(resultsQuery.error)} /> : null}

      <table className="w-full border-collapse text-sm">
        <thead>
          <tr className="border-b border-neutral-200 text-left text-xs font-medium uppercase text-neutral-500">
            <th className="w-6 py-2" aria-hidden="true" />
            <th className="py-2">Subject</th>
            <th className="py-2">Category</th>
            <th className="py-2">Next date</th>
            <th className="py-2">Priority</th>
          </tr>
        </thead>
        <tbody>
          {items.map((result) => (
            <ResultRow
              key={result.id}
              result={result}
              expanded={expandedId === result.id}
              onToggle={() => setExpandedId((id) => (id === result.id ? null : result.id))}
              onSetSeen={(id, s) => seen.setSeen(id, s)}
              pending={seen.isPending(result.id)}
              error={seen.erroredId === result.id ? seen.error : null}
            />
          ))}
        </tbody>
      </table>

      {items.length === 0 && !resultsQuery.isLoading ? (
        <p className="text-sm text-neutral-500">No results match these filters.</p>
      ) : null}

      <div className="flex items-center justify-between text-sm text-neutral-600">
        <span>
          Page {page + 1} of {totalPages} ({total} results)
        </span>
        <div className="flex gap-2">
          <button
            type="button"
            onClick={() => setPage((p) => Math.max(0, p - 1))}
            disabled={page === 0}
            className="rounded border border-neutral-300 px-3 py-1 disabled:opacity-40"
          >
            Previous
          </button>
          <button
            type="button"
            onClick={() => setPage((p) => (p + 1 < totalPages ? p + 1 : p))}
            disabled={page + 1 >= totalPages}
            className="rounded border border-neutral-300 px-3 py-1 disabled:opacity-40"
          >
            Next
          </button>
        </div>
      </div>

      <FiltersModal
        open={filtersOpen}
        onClose={() => setFiltersOpen(false)}
        filters={filters}
        sort={sort}
        accounts={accountsQuery.data ?? []}
        universities={catalogQuery.data ?? []}
        onApply={handleApplyFilters}
      />
    </div>
  );
}
