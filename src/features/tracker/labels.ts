import { formatShortDate } from '../../components/results/format';
import type { EffectiveStatus, ItemChangeDto, ItemKind, LmsItemDto } from '../../lib/api/types';

export const KIND_LABELS: Record<ItemKind, string> = {
  assignment: 'Assignment',
  quiz: 'Quiz',
  test: 'Test',
  event: 'Event',
  other: 'Other',
};

export const STATUS_LABELS: Record<EffectiveStatus, string> = {
  pending: 'Pending',
  overdue: 'Overdue',
  submitted: 'Submitted',
  graded: 'Graded',
  past_event: 'Past',
};

/** The course's short name, or its full name when it has none. */
export function courseLabel(item: Pick<LmsItemDto, 'courseName' | 'courseShortName'>): string {
  return item.courseShortName ?? item.courseName;
}

/** Human-readable description of an item's unseen change; `dueAt` is the item's current due date. */
export function changeText(change: ItemChangeDto, dueAt: string | null): string {
  if (change.kind === 'new') return 'New';
  const previous = change.previousDueAt ? formatShortDate(change.previousDueAt) : 'no date';
  const current = dueAt ? formatShortDate(dueAt) : 'no date';
  return `Date changed: ${previous} → ${current}`;
}
