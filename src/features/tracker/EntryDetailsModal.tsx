import type { TrackerEntry } from './entries';
import ItemDetailsModal from './ItemDetailsModal';
import PersonalTaskModal from './PersonalTaskModal';

export interface EntryDetailsModalProps {
  entry: TrackerEntry | null;
  onClose: () => void;
}

/** Details for the open entry: the item view for a platform entry, the task editor for a personal one. */
export default function EntryDetailsModal({ entry, onClose }: EntryDetailsModalProps) {
  const task = entry?.kind === 'personal' ? entry.task : null;
  return (
    <>
      <ItemDetailsModal item={entry?.kind === 'platform' ? entry.item : null} onClose={onClose} />
      <PersonalTaskModal open={task !== null} task={task} initialDate={task?.date ?? ''} onClose={onClose} />
    </>
  );
}
