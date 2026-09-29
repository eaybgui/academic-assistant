import Modal from '../Modal';
import ErrorMessage from '../ErrorMessage';
import type { CommandError } from '../../lib/api/types';

export interface ConfirmDialogProps {
  open: boolean;
  title: string;
  message: string;
  error?: CommandError | null;
  pending?: boolean;
  onConfirm: () => void;
  onCancel: () => void;
}

/**
 * A small `Modal` wrapper for destructive-action confirmations (e.g. deleting an account or a
 * course). `error` renders inline inside the dialog (rather than on the page behind it, which the
 * open dialog covers) and `pending` disables both actions while the confirmed mutation is in
 * flight.
 */
export default function ConfirmDialog({
  open,
  title,
  message,
  error,
  pending = false,
  onConfirm,
  onCancel,
}: ConfirmDialogProps) {
  return (
    <Modal
      open={open}
      title={title}
      onClose={onCancel}
      footer={
        <>
          <button
            type="button"
            disabled={pending}
            onClick={onCancel}
            className="rounded border border-neutral-300 px-3 py-1.5 text-sm text-neutral-700 hover:bg-neutral-50 disabled:opacity-50"
          >
            Cancel
          </button>
          <button
            type="button"
            disabled={pending}
            onClick={onConfirm}
            className="rounded bg-red-600 px-3 py-1.5 text-sm font-medium text-white hover:bg-red-700 disabled:opacity-50"
          >
            {pending ? 'Deleting…' : 'Delete'}
          </button>
        </>
      }
    >
      <div className="flex flex-col gap-2">
        <p className="text-sm text-neutral-700">{message}</p>
        <ErrorMessage error={error ?? null} />
      </div>
    </Modal>
  );
}
