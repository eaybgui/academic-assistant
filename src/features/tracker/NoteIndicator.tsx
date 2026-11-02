/** A small pencil shown next to the title of an entry that has a note. */
export default function NoteIndicator() {
  return (
    <span role="img" aria-label="Has a note" title="Has a note" className="text-xs text-neutral-500">
      &#9998;
    </span>
  );
}
