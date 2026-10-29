import type { EffectiveStatus } from '../../lib/api/types';
import { STATUS_LABELS } from './labels';

const STATUS_STYLES: Record<EffectiveStatus, string> = {
  pending: 'bg-neutral-100 text-neutral-700',
  overdue: 'bg-red-100 text-red-700',
  submitted: 'bg-green-100 text-green-700',
  graded: 'bg-green-100 text-green-700',
  past_event: 'bg-neutral-100 text-neutral-700',
};

/** A small coloured pill with the item's effective status. */
export default function StatusPill({ status }: { status: EffectiveStatus }) {
  return (
    <span className={`rounded-full px-2 py-0.5 text-xs font-medium ${STATUS_STYLES[status]}`}>
      {STATUS_LABELS[status]}
    </span>
  );
}
