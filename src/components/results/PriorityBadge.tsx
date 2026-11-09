import type { Priority } from '../../lib/api/types';
import { PRIORITY_LABELS } from './labels';
import { PILL_BASE, TONE_AMBER, TONE_BLUE, TONE_RED } from '../styles';

const PRIORITY_STYLES: Record<Priority, string> = {
  urgent: TONE_RED,
  high: TONE_AMBER,
  medium: TONE_BLUE,
  low: 'bg-neutral-100 text-neutral-600',
};

export interface PriorityBadgeProps {
  priority: Priority;
}

/** Colored pill for a result's priority: urgent red, high amber, medium blue, low gray. */
export default function PriorityBadge({ priority }: PriorityBadgeProps) {
  return (
    <span
      className={`${PILL_BASE} ${PRIORITY_STYLES[priority]}`}
    >
      {PRIORITY_LABELS[priority]}
    </span>
  );
}
