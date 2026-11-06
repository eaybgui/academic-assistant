import { TEXT_MUTED_XS } from '../../components/styles';
/** A small pencil shown next to the title of an entry that has a note. */
export default function NoteIndicator() {
  return (
    <span role="img" aria-label="Has a note" title="Has a note" className={TEXT_MUTED_XS}>
      &#9998;
    </span>
  );
}
