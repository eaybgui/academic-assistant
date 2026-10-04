import { useState } from 'react';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { api, toCommandError } from '../../lib/api/client';
import { queryKeys } from '../../lib/api/queryKeys';
import ConfirmDialog from '../../components/forms/ConfirmDialog';
import { PencilIcon, TrashIcon } from '../../components/forms/icons';
import type { CommandError, CourseDto, UniversityDto } from '../../lib/api/types';
import UniversityFormModal from './UniversityFormModal';
import CourseFormModal from './CourseFormModal';

type DeleteTarget = { kind: 'university'; university: UniversityDto } | { kind: 'course'; course: CourseDto };

/** Catalog screen: universities with their nested courses, each with add/edit/delete actions. */
export default function CatalogPage() {
  const queryClient = useQueryClient();
  const catalogQuery = useQuery({ queryKey: queryKeys.catalog, queryFn: api.getCatalog });

  const [universityModalOpen, setUniversityModalOpen] = useState(false);
  // Open counters are part of each modal's `key`, so every open starts with fresh form state.
  const [universityOpenCount, setUniversityOpenCount] = useState(0);
  const [editingUniversity, setEditingUniversity] = useState<UniversityDto | null>(null);

  const [courseModalOpen, setCourseModalOpen] = useState(false);
  const [courseOpenCount, setCourseOpenCount] = useState(0);
  const [courseUniversityId, setCourseUniversityId] = useState<number | null>(null);
  const [editingCourse, setEditingCourse] = useState<CourseDto | null>(null);

  const [deleteTarget, setDeleteTarget] = useState<DeleteTarget | null>(null);
  const [rowError, setRowError] = useState<CommandError | null>(null);

  function invalidateCatalog() {
    void queryClient.invalidateQueries({ queryKey: queryKeys.catalog });
  }

  const deleteUniversityMutation = useMutation({
    mutationFn: (id: number) => api.removeUniversity(id),
    onSuccess: () => {
      setDeleteTarget(null);
      setRowError(null);
      invalidateCatalog();
    },
    onError: (e) => setRowError(toCommandError(e)),
  });

  const deleteCourseMutation = useMutation({
    mutationFn: (id: number) => api.removeCourse(id),
    onSuccess: () => {
      setDeleteTarget(null);
      setRowError(null);
      invalidateCatalog();
    },
    onError: (e) => setRowError(toCommandError(e)),
  });

  function openDeleteConfirm(target: DeleteTarget) {
    setRowError(null);
    setDeleteTarget(target);
  }

  function closeDeleteConfirm() {
    setDeleteTarget(null);
    setRowError(null);
  }

  function openAddUniversity() {
    setEditingUniversity(null);
    setUniversityOpenCount((count) => count + 1);
    setUniversityModalOpen(true);
  }

  function openEditUniversity(university: UniversityDto) {
    setEditingUniversity(university);
    setUniversityOpenCount((count) => count + 1);
    setUniversityModalOpen(true);
  }

  function openAddCourse(universityId: number) {
    setCourseUniversityId(universityId);
    setEditingCourse(null);
    setCourseOpenCount((count) => count + 1);
    setCourseModalOpen(true);
  }

  function openEditCourse(universityId: number, course: CourseDto) {
    setCourseUniversityId(universityId);
    setEditingCourse(course);
    setCourseOpenCount((count) => count + 1);
    setCourseModalOpen(true);
  }

  function handleConfirmDelete() {
    if (!deleteTarget) return;
    if (deleteTarget.kind === 'university') {
      deleteUniversityMutation.mutate(deleteTarget.university.id);
    } else {
      deleteCourseMutation.mutate(deleteTarget.course.id);
    }
  }

  const universities = catalogQuery.data ?? [];

  return (
    <div className="flex flex-col gap-4">
      <div className="flex items-center justify-between">
        <h1 className="text-2xl font-semibold text-neutral-900">University catalog</h1>
        <button
          type="button"
          onClick={openAddUniversity}
          className="rounded bg-neutral-900 px-3 py-1.5 text-sm font-medium text-white hover:bg-neutral-800"
        >
          Add university
        </button>
      </div>

      {catalogQuery.isLoading ? <p className="text-sm text-neutral-500">Loading…</p> : null}
      {universities.length === 0 && !catalogQuery.isLoading ? (
        <p className="text-sm text-neutral-500">No universities yet. Add one to organize your courses.</p>
      ) : null}

      <ul className="flex flex-col gap-3">
        {universities.map((university) => (
          <li key={university.id} className="rounded border border-neutral-200 bg-white px-4 py-3">
            <div className="flex items-center justify-between">
              <p className="font-bold text-neutral-900">{university.name}</p>
              <div className="flex items-center gap-3 text-sm text-neutral-700">
                <button type="button" onClick={() => openEditUniversity(university)} className="hover:underline">
                  Edit
                </button>
                <span className="text-neutral-300">·</span>
                <button
                  type="button"
                  onClick={() => openAddCourse(university.id)}
                  className="hover:underline"
                >
                  Add course
                </button>
                <button
                  type="button"
                  aria-label={`Delete ${university.name}`}
                  onClick={() => openDeleteConfirm({ kind: 'university', university })}
                  className="text-neutral-400 hover:text-red-600"
                >
                  <TrashIcon />
                </button>
              </div>
            </div>

            {university.courses.length > 0 ? (
              <ul className="mt-2 flex flex-col gap-1 border-t border-neutral-100 pt-2">
                {university.courses.map((course) => (
                  <li key={course.id} className="flex items-center justify-between text-sm text-neutral-700">
                    <span>
                      {course.name}
                      {course.aliases.length > 0 ? (
                        <span className="text-neutral-500"> · aliases: {course.aliases.join(', ')}</span>
                      ) : null}
                    </span>
                    <span className="flex items-center gap-2">
                      <button
                        type="button"
                        aria-label={`Edit ${course.name}`}
                        onClick={() => openEditCourse(university.id, course)}
                        className="text-neutral-400 hover:text-neutral-700"
                      >
                        <PencilIcon />
                      </button>
                      <button
                        type="button"
                        aria-label={`Delete ${course.name}`}
                        onClick={() => openDeleteConfirm({ kind: 'course', course })}
                        className="text-neutral-400 hover:text-red-600"
                      >
                        <TrashIcon />
                      </button>
                    </span>
                  </li>
                ))}
              </ul>
            ) : null}
          </li>
        ))}
      </ul>

      <UniversityFormModal
        key={`${editingUniversity?.id ?? 'new'}-${universityOpenCount}`}
        open={universityModalOpen}
        university={editingUniversity}
        onClose={() => setUniversityModalOpen(false)}
      />

      {courseUniversityId !== null ? (
        <CourseFormModal
          key={`${editingCourse?.id ?? 'new'}-${courseOpenCount}`}
          open={courseModalOpen}
          universityId={courseUniversityId}
          course={editingCourse}
          onClose={() => setCourseModalOpen(false)}
        />
      ) : null}

      <ConfirmDialog
        open={deleteTarget !== null}
        title={deleteTarget?.kind === 'university' ? 'Delete university' : 'Delete course'}
        message={
          deleteTarget?.kind === 'university'
            ? 'Deletes this university and all its courses.'
            : 'Deletes this course.'
        }
        error={rowError}
        pending={deleteUniversityMutation.isPending || deleteCourseMutation.isPending}
        onCancel={closeDeleteConfirm}
        onConfirm={handleConfirmDelete}
      />
    </div>
  );
}
