import type { ResultQuery, ResultSort } from './types';

/**
 * Stable React Query keys for the API client. Keeping them in one place avoids
 * subtly mismatched keys (e.g. typos) between the query and its invalidations.
 */
export const queryKeys = {
  dashboard: (sort: ResultSort) => ['dashboard', sort] as const,
  results: (query: ResultQuery) => ['results', query] as const,
  runs: (limit: number) => ['runs', limit] as const,
  accounts: ['accounts'] as const,
  catalog: ['catalog'] as const,
  universities: ['universities'] as const,
  settings: ['settings'] as const,
};
