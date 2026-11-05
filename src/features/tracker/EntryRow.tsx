import type { TrackerEntry } from './entries';
import ItemRow from './ItemRow';
import PersonalTaskRow from './PersonalTaskRow';

export interface EntryRowProps {
  entry: TrackerEntry;
  showUniversity: boolean;
  onOpen: (entry: TrackerEntry) => void;
}

/** A tracker row for either kind of entry. */
export default function EntryRow({ entry, showUniversity, onOpen }: EntryRowProps) {
  return entry.kind === 'platform' ? (
    <ItemRow item={entry.item} showUniversity={showUniversity} onOpen={() => onOpen(entry)} />
  ) : (
    <PersonalTaskRow entry={entry} onOpen={onOpen} />
  );
}
