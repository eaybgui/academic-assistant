import type { EntryStatus } from './entries';
import { STATUS_LABELS } from './labels';
import { TONE_GREEN, TONE_NEUTRAL, TONE_RED } from '../../components/styles';

const STATUS_STYLES: Record<EntryStatus, string> = {
  pending: TONE_NEUTRAL,
  overdue: TONE_RED,
  submitted: TONE_GREEN,
  graded: TONE_GREEN,
  past_event: TONE_NEUTRAL,
  done: TONE_GREEN,
};

/** A small coloured pill with the entry's status. */
export default function StatusPill({ status }: { status: EntryStatus }) {
  return (
    <span className={`rounded-full px-2 py-0.5 text-xs font-medium ${STATUS_STYLES[status]}`}>
      {STATUS_LABELS[status]}
    </span>
  );
}
