import { useState } from 'react';
import { useMutation } from '@tanstack/react-query';
import Modal from '../../components/Modal';
import ErrorMessage from '../../components/ErrorMessage';
import Field from '../../components/forms/Field';
import TextInput from '../../components/forms/TextInput';
import { api, toCommandError } from '../../lib/api/client';
import type { CommandError, LmsUniversityDto } from '../../lib/api/types';
import { useInvalidateUniversities } from './useInvalidateUniversities';
import { PRIMARY_BUTTON, SECONDARY_BUTTON_BASE } from '../../components/styles';

export interface RenameUniversityModalProps {
  open: boolean;
  university: LmsUniversityDto;
  onClose: () => void;
}

/** Modal with a single Name field to rename a university. */
export default function RenameUniversityModal({ open, university, onClose }: RenameUniversityModalProps) {
  const [name, setName] = useState(university.name);
  const [validationError, setValidationError] = useState<string | null>(null);
  const [saveError, setSaveError] = useState<CommandError | null>(null);
  const invalidate = useInvalidateUniversities();

  const saveMutation = useMutation({
    mutationFn: () => api.renameUniversity(university.id, name.trim()),
    onSuccess: () => {
      invalidate();
      onClose();
    },
    onError: (e) => setSaveError(toCommandError(e)),
  });

  function handleSubmit() {
    if (!name.trim()) {
      setValidationError('University name is required.');
      return;
    }
    setValidationError(null);
    setSaveError(null);
    saveMutation.mutate();
  }

  return (
    <Modal
      open={open}
      title="Rename university"
      onClose={onClose}
      footer={
        <>
          <button
            type="button"
            onClick={onClose}
            className={SECONDARY_BUTTON_BASE}
          >
            Cancel
          </button>
          <button
            type="button"
            disabled={saveMutation.isPending}
            onClick={handleSubmit}
            className={PRIMARY_BUTTON}
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
