import { useState } from 'react';
import { openUrl } from '@tauri-apps/plugin-opener';
import Modal from '../../components/Modal';
import ErrorMessage from '../../components/ErrorMessage';
import Field from '../../components/forms/Field';
import Textarea from '../../components/forms/Textarea';
import { formatDate, formatDateTime } from '../../components/results/format';
import { toCommandError } from '../../lib/api/client';
import type { LmsItemDto } from '../../lib/api/types';
import StatusPill from './StatusPill';
import { KIND_LABELS, canMarkDone, changeText } from './labels';
import { useItemMutations } from './useItemMutations';

export interface ItemDetailsModalProps {
  item: LmsItemDto | null;
  onClose: () => void;
}

const ACTION_BUTTON =
  'rounded border border-neutral-300 px-3 py-1.5 text-sm text-neutral-700 hover:bg-neutral-50 disabled:opacity-50';

/** Full detail view of a tracker item: metadata, platform link, note editor and the hide / done actions. */
export default function ItemDetailsModal({ item, onClose }: ItemDetailsModalProps) {
  return (
    <Modal open={item !== null} title={item?.title ?? ''} onClose={onClose}>
      {item ? <ItemDetails key={item.id} item={item} /> : null}
    </Modal>
  );
}

function ItemDetails({ item }: { item: LmsItemDto }) {
  const { setDone, setHidden, setNote } = useItemMutations();
  const [note, setNoteText] = useState(item.note ?? '');
  const [openError, setOpenError] = useState(false);
  const canOpen = item.url?.startsWith('https://') ?? false;
  const actionError = [setDone, setHidden].find((mutation) => mutation.isError);

  async function handleOpen() {
    if (!item.url) return;
    try {
      await openUrl(item.url);
      setOpenError(false);
    } catch {
      setOpenError(true);
    }
  }

  return (
    <div className="flex flex-col gap-3 text-sm text-neutral-700">
      <p className="text-neutral-500">
        {item.universityName} · {item.courseName} · {KIND_LABELS[item.kind]}
      </p>
      <div className="flex flex-wrap items-center gap-2">
        <StatusPill status={item.status} />
        {item.grade ? <span className="font-medium text-neutral-900">{item.grade.display}</span> : null}
      </div>
      <p>{item.dueAt ? `Due ${formatDateTime(item.dueAt)}` : 'No date'}</p>
      {item.change ? <p className="text-amber-800">{changeText(item.change, item.dueAt)}</p> : null}
      {item.removedAt ? <p>Removed from the platform on {formatDate(item.removedAt)}</p> : null}
      {canOpen ? (
        <div>
          <button type="button" onClick={() => void handleOpen()} className={ACTION_BUTTON}>
            Open in platform
          </button>
          {openError ? <p className="mt-1 text-sm text-red-600">Couldn&apos;t open the link</p> : null}
        </div>
      ) : null}

      <Field label="Note" htmlFor="item-note">
        <Textarea id="item-note" rows={4} value={note} onChange={(e) => setNoteText(e.target.value)} />
      </Field>
      <div className="flex items-center gap-3">
        <button
          type="button"
          disabled={setNote.isPending}
          onClick={() => setNote.mutate({ id: item.id, note: note.trim() === '' ? null : note })}
          className={ACTION_BUTTON}
        >
          Save note
        </button>
        <ErrorMessage error={setNote.isError ? toCommandError(setNote.error) : null} />
      </div>

      <div className="flex flex-wrap gap-2 border-t border-neutral-200 pt-3">
        <button
          type="button"
          disabled={setHidden.isPending}
          onClick={() => setHidden.mutate({ id: item.id, value: !item.hidden })}
          className={ACTION_BUTTON}
        >
          {item.hidden ? 'Show again' : 'Hide'}
        </button>
        {canMarkDone(item) ? (
          <button
            type="button"
            disabled={setDone.isPending}
            onClick={() => setDone.mutate({ id: item.id, value: !item.done })}
            className={ACTION_BUTTON}
          >
            {item.done ? 'Mark as not done' : 'Mark as done'}
          </button>
        ) : null}
      </div>
      <ErrorMessage error={actionError ? toCommandError(actionError.error) : null} />
    </div>
  );
}
