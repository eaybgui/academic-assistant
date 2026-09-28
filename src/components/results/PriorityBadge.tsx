import type { Priority } from '../../lib/api/types';
import { PRIORITY_LABELS } from './labels';

const PRIORITY_STYLES: Record<Priority, string> = {
  urgent: 'bg-red-100 text-red-700',
  high: 'bg-amber-100 text-amber-800',
  medium: 'bg-blue-100 text-blue-700',
  low: 'bg-neutral-100 text-neutral-600',
};

export interface PriorityBadgeProps {
  priority: Priority;
}

/** Colored pill for a result's priority: urgent red, high amber, medium blue, low gray. */
export default function PriorityBadge({ priority }: PriorityBadgeProps) {
  return (
    <span
      className={`inline-flex items-center rounded-full px-2 py-0.5 text-xs font-medium ${PRIORITY_STYLES[priority]}`}
    >
      {PRIORITY_LABELS[priority]}
    </span>
  );
}
