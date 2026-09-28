import { useState } from 'react';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import Modal from '../../components/Modal';
import ErrorMessage from '../../components/ErrorMessage';
import Field from '../../components/forms/Field';
import TextInput from '../../components/forms/TextInput';
import { api, toCommandError } from '../../lib/api/client';
import { queryKeys } from '../../lib/api/queryKeys';
import type { CommandError, CourseDto } from '../../lib/api/types';
import { parseAliases } from './catalogForm';

export interface CourseFormModalProps {
  open: boolean;
  universityId: number;
  course: CourseDto | null;
  onClose: () => void;
}

/** Add/edit modal for a course within a university, including its comma-separated aliases. */
export default function CourseFormModal({ open, universityId, course, onClose }: CourseFormModalProps) {
  const isCreate = course === null;
  const [name, setName] = useState(course?.name ?? '');
  const [aliases, setAliases] = useState(course?.aliases.join(', ') ?? '');
  const [validationError, setValidationError] = useState<string | null>(null);
  const [saveError, setSaveError] = useState<CommandError | null>(null);
  const queryClient = useQueryClient();

  const saveMutation = useMutation({
    mutationFn: () => {
      const aliasList = parseAliases(aliases);
      return course
        ? api.updateCourse(course.id, name.trim(), aliasList)
        : api.addCourse(universityId, name.trim(), aliasList);
    },
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: queryKeys.catalog });
      handleClose();
    },
    onError: (e) => setSaveError(toCommandError(e)),
  });

  function handleClose() {
    setName(course?.name ?? '');
    setAliases(course?.aliases.join(', ') ?? '');
    setValidationError(null);
    setSaveError(null);
    onClose();
  }

  function handleSubmit() {
    if (!name.trim()) {
      setValidationError('Course name is required.');
      return;
    }
    setValidationError(null);
    saveMutation.mutate();
  }

  return (
    <Modal
      open={open}
      title={isCreate ? 'Add course' : 'Edit course'}
      onClose={handleClose}
      footer={
        <>
          <button
            type="button"
            onClick={handleClose}
            className="rounded border border-neutral-300 px-3 py-1.5 text-sm text-neutral-700 hover:bg-neutral-50"
          >
            Cancel
          </button>
          <button
            type="button"
            disabled={saveMutation.isPending}
            onClick={handleSubmit}
            className="rounded bg-neutral-900 px-3 py-1.5 text-sm font-medium text-white hover:bg-neutral-800 disabled:opacity-50"
          >
            {saveMutation.isPending ? 'Saving…' : 'Save'}
          </button>
        </>
      }
    >
      <div className="flex flex-col gap-3">
        <Field label="Name" htmlFor="course-name" error={validationError ?? undefined}>
          <TextInput id="course-name" value={name} onChange={(e) => setName(e.target.value)} />
        </Field>
        <Field label="Aliases" htmlFor="course-aliases" hint="Comma-separated, e.g. Algebra, Linear Algebra I">
          <TextInput id="course-aliases" value={aliases} onChange={(e) => setAliases(e.target.value)} />
        </Field>
        <ErrorMessage error={saveError} />
      </div>
    </Modal>
  );
}
