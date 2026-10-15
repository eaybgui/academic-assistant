import { useState } from 'react';
import { useMutation, useQuery } from '@tanstack/react-query';
import Modal from '../../components/Modal';
import ErrorMessage from '../../components/ErrorMessage';
import { api, toCommandError } from '../../lib/api/client';
import type { LmsUniversityDto } from '../../lib/api/types';
import { useSync } from '../sync/useSync';
import CourseSelectionList from './CourseSelectionList';
import { useInvalidateUniversities } from './useInvalidateUniversities';

export interface EditCoursesModalProps {
  open: boolean;
  university: LmsUniversityDto;
  onClose: () => void;
}

/**
 * Chooses which courses of a university are tracked. The list is refreshed from the platform; when
 * that fails (e.g. the session expired) the stored list is shown together with the error. Saving
 * starts a sync of the university; a sync problem is reported by the dashboard, not here.
 */
export default function EditCoursesModal({ open, university, onClose }: EditCoursesModalProps) {
  const { id } = university;
  const { startSync } = useSync();
  const invalidate = useInvalidateUniversities();
  const [selection, setSelection] = useState<Set<number> | null>(null);

  const coursesQuery = useQuery({
    queryKey: ['edit-courses', id],
    queryFn: async () => {
      try {
        return { courses: await api.lmsFetchCourses(id), fetchError: null };
      } catch (e) {
        return { courses: await api.getCourses(id), fetchError: toCommandError(e) };
      }
    },
    enabled: open,
    retry: false,
    gcTime: 0,
    refetchOnWindowFocus: false,
  });

  const courses = coursesQuery.data?.courses ?? [];
  const selected = selection ?? new Set(courses.filter((c) => c.tracked).map((c) => c.id));

  const saveMutation = useMutation({
    mutationFn: async () => {
      await api.setTrackedCourses(id, [...selected]);
      await startSync(id);
    },
    onSuccess: () => {
      invalidate();
      onClose();
    },
  });

  return (
    <Modal
      open={open}
      title={`Edit courses of ${university.name}`}
      onClose={onClose}
      footer={
        <>
          <button
            type="button"
            onClick={onClose}
            className="rounded border border-neutral-300 px-3 py-1.5 text-sm text-neutral-700 hover:bg-neutral-50"
          >
            Cancel
          </button>
          <button
            type="button"
            disabled={!coursesQuery.data || saveMutation.isPending}
            onClick={() => saveMutation.mutate()}
            className="rounded bg-neutral-900 px-3 py-1.5 text-sm font-medium text-white hover:bg-neutral-800 disabled:opacity-50"
          >
            {saveMutation.isPending ? 'Saving…' : 'Save'}
          </button>
        </>
      }
    >
      <div className="flex flex-col gap-3">
        {coursesQuery.isPending ? <p className="text-sm text-neutral-500">Loading courses…</p> : null}
        <ErrorMessage error={coursesQuery.data?.fetchError ?? null} />
        <ErrorMessage error={coursesQuery.error ? toCommandError(coursesQuery.error) : null} />
        {coursesQuery.data ? (
          <CourseSelectionList courses={courses} selected={selected} onChange={setSelection} />
        ) : null}
        <ErrorMessage error={saveMutation.error ? toCommandError(saveMutation.error) : null} />
      </div>
    </Modal>
  );
}
