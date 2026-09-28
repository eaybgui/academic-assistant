import type { Category } from '../../lib/api/types';

export interface ResultFilters {
  seen?: boolean;
  category?: Category;
  accountId?: number;
  courseId?: number;
}

/** Counts the active (non-sort) filters, for the "Filters and sort" badge. */
export function countActiveFilters(filters: ResultFilters): number {
  return Object.values(filters).filter((value) => value !== undefined).length;
}
