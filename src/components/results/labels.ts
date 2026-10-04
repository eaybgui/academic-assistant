import type { Category, DateKind, Priority, ResultSort } from '../../lib/api/types';

/** Human labels for `Category`, used everywhere a result's category is shown. */
export const CATEGORY_LABELS: Record<Category, string> = {
  notice: 'Notice',
  task: 'Task',
  exam: 'Exam',
  event: 'Event',
  change: 'Change',
  general_info: 'General info',
};

/** Human labels for `DateKind`, used for each date attached to a result. */
export const DATE_KIND_LABELS: Record<DateKind, string> = {
  exam: 'Exam',
  deadline: 'Deadline',
  event: 'Event',
  class: 'Class',
  other: 'Other',
};

/** Human labels for `Priority`, used by `PriorityBadge`. */
export const PRIORITY_LABELS: Record<Priority, string> = {
  low: 'Low',
  medium: 'Medium',
  high: 'High',
  urgent: 'Urgent',
};

/** Human labels for `ResultSort`, used by `SortSelect`. */
export const RESULT_SORT_LABELS: Record<ResultSort, string> = {
  upcoming: 'Nearest upcoming date',
  priority: 'Priority',
  received: 'Received date',
};
