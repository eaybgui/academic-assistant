import { useState } from 'react';
import Modal from '../../components/Modal';
import SortSelect from '../../components/results/SortSelect';
import { CATEGORY_LABELS } from '../../components/results/labels';
import type { AccountDto, Category, ResultSort, UniversityDto } from '../../lib/api/types';
import type { ResultFilters } from './filterUtils';

export interface FiltersModalProps {
  open: boolean;
  onClose: () => void;
  filters: ResultFilters;
  sort: ResultSort;
  accounts: AccountDto[];
  universities: UniversityDto[];
  onApply: (filters: ResultFilters, sort: ResultSort) => void;
}

const CATEGORY_OPTIONS: Category[] = ['notice', 'task', 'exam', 'event', 'change', 'general_info'];

/**
 * Modal for the Results page's "Filters and sort" button: state (seen/new),
 * category, account, course and sort. Edits are local until "Apply"; "Reset"
 * clears the draft back to defaults without closing the modal.
 */
export default function FiltersModal({
  open,
  onClose,
  filters,
  sort,
  accounts,
  universities,
  onApply,
}: FiltersModalProps) {
  const [draftFilters, setDraftFilters] = useState<ResultFilters>(filters);
  const [draftSort, setDraftSort] = useState<ResultSort>(sort);
  // Tracks the `open` value the draft was last synced for. Re-syncing the
  // draft from the committed filters/sort on open is a state adjustment in
  // response to a prop change, done during render (React's documented
  // alternative to an Effect for this), not a side effect — so it doesn't
  // cost an extra render and can't clobber in-progress edits while open.
  const [syncedFor, setSyncedFor] = useState(open);
  if (open !== syncedFor) {
    setSyncedFor(open);
    if (open) {
      setDraftFilters(filters);
      setDraftSort(sort);
    }
  }

  const courses = universities.flatMap((university) =>
    university.courses.map((course) => ({ id: course.id, label: `${course.name} (${university.name})` })),
  );

  function handleReset() {
    setDraftFilters({});
    setDraftSort('upcoming');
  }

  function handleApply() {
    onApply(draftFilters, draftSort);
    onClose();
  }

  return (
    <Modal
      open={open}
      title="Filters and sort"
      onClose={onClose}
      footer={
        <>
          <button
            type="button"
            onClick={handleReset}
            className="rounded px-3 py-1.5 text-sm text-neutral-600 hover:bg-neutral-100"
          >
            Reset
          </button>
          <button
            type="button"
            onClick={handleApply}
            className="rounded bg-neutral-900 px-3 py-1.5 text-sm font-medium text-white hover:bg-neutral-800"
          >
            Apply
          </button>
        </>
      }
    >
      <div className="space-y-4">
        <div>
          <label htmlFor="filter-state" className="mb-1 block text-sm font-medium text-neutral-700">
            State
          </label>
          <select
            id="filter-state"
            value={draftFilters.seen === undefined ? '' : String(draftFilters.seen)}
            onChange={(e) =>
              setDraftFilters((f) => ({
                ...f,
                seen: e.target.value === '' ? undefined : e.target.value === 'true',
              }))
            }
            className="w-full rounded border border-neutral-300 px-2 py-1.5 text-sm"
          >
            <option value="">All</option>
            <option value="false">New</option>
            <option value="true">Seen</option>
          </select>
        </div>

        <div>
          <label htmlFor="filter-category" className="mb-1 block text-sm font-medium text-neutral-700">
            Category
          </label>
          <select
            id="filter-category"
            value={draftFilters.category ?? ''}
            onChange={(e) =>
              setDraftFilters((f) => ({
                ...f,
                category: e.target.value === '' ? undefined : (e.target.value as Category),
              }))
            }
            className="w-full rounded border border-neutral-300 px-2 py-1.5 text-sm"
          >
            <option value="">All</option>
            {CATEGORY_OPTIONS.map((category) => (
              <option key={category} value={category}>
                {CATEGORY_LABELS[category]}
              </option>
            ))}
          </select>
        </div>

        <div>
          <label htmlFor="filter-account" className="mb-1 block text-sm font-medium text-neutral-700">
            Account
          </label>
          <select
            id="filter-account"
            value={draftFilters.accountId ?? ''}
            onChange={(e) =>
              setDraftFilters((f) => ({
                ...f,
                accountId: e.target.value === '' ? undefined : Number(e.target.value),
              }))
            }
            className="w-full rounded border border-neutral-300 px-2 py-1.5 text-sm"
          >
            <option value="">All</option>
            {accounts.map((account) => (
              <option key={account.id} value={account.id}>
                {account.name}
              </option>
            ))}
          </select>
        </div>

        <div>
          <label htmlFor="filter-course" className="mb-1 block text-sm font-medium text-neutral-700">
            Course
          </label>
          <select
            id="filter-course"
            value={draftFilters.courseId ?? ''}
            onChange={(e) =>
              setDraftFilters((f) => ({
                ...f,
                courseId: e.target.value === '' ? undefined : Number(e.target.value),
              }))
            }
            className="w-full rounded border border-neutral-300 px-2 py-1.5 text-sm"
          >
            <option value="">All</option>
            {courses.map((course) => (
              <option key={course.id} value={course.id}>
                {course.label}
              </option>
            ))}
          </select>
        </div>

        <div>
          <label htmlFor="filter-sort" className="mb-1 block text-sm font-medium text-neutral-700">
            Sort
          </label>
          <SortSelect id="filter-sort" value={draftSort} onChange={setDraftSort} />
        </div>
      </div>
    </Modal>
  );
}
