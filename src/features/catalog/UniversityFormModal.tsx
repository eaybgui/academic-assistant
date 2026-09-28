import { useState } from 'react';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import Modal from '../../components/Modal';
import ErrorMessage from '../../components/ErrorMessage';
import Field from '../../components/forms/Field';
import TextInput from '../../components/forms/TextInput';
import { api, toCommandError } from '../../lib/api/client';
import { queryKeys } from '../../lib/api/queryKeys';
import type { CommandError, UniversityDto } from '../../lib/api/types';

export interface UniversityFormModalProps {
  open: boolean;
  university: UniversityDto | null;
  onClose: () => void;
}

/** Add/rename modal for a university. */
export default function UniversityFormModal({ open, university, onClose }: UniversityFormModalProps) {
  const isCreate = university === null;
  const [name, setName] = useState(university?.name ?? '');
  const [validationError, setValidationError] = useState<string | null>(null);
  const [saveError, setSaveError] = useState<CommandError | null>(null);
  const queryClient = useQueryClient();

  const saveMutation = useMutation({
    mutationFn: () =>
      university ? api.updateUniversity(university.id, name.trim()) : api.addUniversity(name.trim()),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: queryKeys.catalog });
      handleClose();
    },
    onError: (e) => setSaveError(toCommandError(e)),
  });

  function handleClose() {
    setName(university?.name ?? '');
    setValidationError(null);
    setSaveError(null);
    onClose();
  }

  function handleSubmit() {
    if (!name.trim()) {
      setValidationError('University name is required.');
      return;
    }
    setValidationError(null);
    saveMutation.mutate();
  }

  return (
    <Modal
      open={open}
      title={isCreate ? 'Add university' : 'Edit university'}
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
        <Field label="Name" htmlFor="university-name" error={validationError ?? undefined}>
          <TextInput id="university-name" value={name} onChange={(e) => setName(e.target.value)} />
        </Field>
        <ErrorMessage error={saveError} />
      </div>
    </Modal>
  );
}
