import { useState } from 'react';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import Modal from '../../components/Modal';
import ErrorMessage from '../../components/ErrorMessage';
import Field from '../../components/forms/Field';
import TextInput from '../../components/forms/TextInput';
import { api, toCommandError } from '../../lib/api/client';
import { queryKeys } from '../../lib/api/queryKeys';
import type { CommandError } from '../../lib/api/types';

export interface JevKeyModalProps {
  open: boolean;
  onClose: () => void;
}

/**
 * Replaces the jev API key. The key is only ever sent to `setJevApiKey`; it is never rendered
 * back, before or after saving.
 */
export default function JevKeyModal({ open, onClose }: JevKeyModalProps) {
  const [key, setKey] = useState('');
  const [validationError, setValidationError] = useState<string | null>(null);
  const [saveError, setSaveError] = useState<CommandError | null>(null);
  const queryClient = useQueryClient();

  const saveMutation = useMutation({
    mutationFn: () => api.setJevApiKey(key),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: queryKeys.settings });
      handleClose();
    },
    onError: (e) => setSaveError(toCommandError(e)),
  });

  function handleClose() {
    setKey('');
    setValidationError(null);
    setSaveError(null);
    onClose();
  }

  function handleSubmit() {
    if (!key) {
      setValidationError('Enter an API key.');
      return;
    }
    setValidationError(null);
    saveMutation.mutate();
  }

  return (
    <Modal
      open={open}
      title="Replace jev API key"
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
        <Field label="API key" htmlFor="jev-api-key" error={validationError ?? undefined}>
          <TextInput
            id="jev-api-key"
            type="password"
            autoComplete="off"
            value={key}
            onChange={(e) => setKey(e.target.value)}
          />
        </Field>
        <ErrorMessage error={saveError} />
      </div>
    </Modal>
  );
}
