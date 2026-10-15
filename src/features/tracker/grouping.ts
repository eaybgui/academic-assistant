import type { LmsItemDto } from '../../lib/api/types';

export type TimeGroupKey = 'overdue' | 'today' | 'this_week' | 'next_week' | 'later' | 'no_date';
export type StatusGroupKey = 'pending' | 'submitted' | 'graded' | 'past_events';
export interface ItemGroup<K extends string> {
  key: K;
  label: string;
  items: LmsItemDto[];
}
export interface GroupedItems<K extends string> {
  groups: ItemGroup<K>[];
  completed: LmsItemDto[];
}

const COLLATOR = new Intl.Collator(undefined, { sensitivity: 'base', numeric: true });
const TIME_LABELS: Record<TimeGroupKey, string> = {
  overdue: 'Overdue',
  today: 'Today',
  this_week: 'This week',
  next_week: 'Next week',
  later: 'Later',
  no_date: 'No date',
};
const TIME_ORDER: TimeGroupKey[] = ['overdue', 'today', 'this_week', 'next_week', 'later', 'no_date'];
const STATUS_LABELS: Record<StatusGroupKey, string> = {
  pending: 'Pending',
  submitted: 'Submitted',
  graded: 'Graded',
  past_events: 'Past events',
};

export function isCompleted(item: LmsItemDto): boolean {
  return item.status === 'submitted' || item.status === 'graded' || item.status === 'past_event';
}

export function compareItems(a: LmsItemDto, b: LmsItemDto): number {
  if (a.dueAt !== b.dueAt) {
    if (a.dueAt === null) return 1;
    if (b.dueAt === null) return -1;
    const diff = Date.parse(a.dueAt) - Date.parse(b.dueAt);
    if (diff !== 0) return diff;
  }
  return COLLATOR.compare(a.title, b.title) || a.id - b.id;
}

function startOfDay(d: Date): Date {
  return new Date(d.getFullYear(), d.getMonth(), d.getDate());
}

function addDays(d: Date, days: number): Date {
  return new Date(d.getFullYear(), d.getMonth(), d.getDate() + days);
}

function timeKey(item: LmsItemDto, now: Date): TimeGroupKey {
  if (item.dueAt === null) return 'no_date';
  if (item.status === 'overdue') return 'overdue';
  const due = new Date(item.dueAt);
  const today = startOfDay(now);
  const tomorrow = addDays(today, 1);
  const daysToMonday = (8 - today.getDay()) % 7 || 7; // getDay: Sunday 0 … Saturday 6
  const nextMonday = addDays(today, daysToMonday);
  const mondayAfter = addDays(nextMonday, 7);
  if (due < tomorrow) return 'today';
  if (due < nextMonday) return 'this_week';
  if (due < mondayAfter) return 'next_week';
  return 'later';
}

function collect<K extends string>(
  order: K[],
  labels: Record<K, string>,
  keyed: [K, LmsItemDto][],
): ItemGroup<K>[] {
  return order
    .map((key) => ({
      key,
      label: labels[key],
      items: keyed
        .filter(([k]) => k === key)
        .map(([, i]) => i)
        .sort(compareItems),
    }))
    .filter((group) => group.items.length > 0);
}

export function groupByTime(items: LmsItemDto[], now: Date): GroupedItems<TimeGroupKey> {
  const open = items.filter((i) => !isCompleted(i));
  const completed = items.filter(isCompleted).sort(compareItems);
  return {
    groups: collect(
      TIME_ORDER,
      TIME_LABELS,
      open.map((i): [TimeGroupKey, LmsItemDto] => [timeKey(i, now), i]),
    ),
    completed,
  };
}

export function groupByStatus(items: LmsItemDto[]): GroupedItems<StatusGroupKey> {
  const pending = items.filter((i) => i.status === 'pending' || i.status === 'overdue');
  const overdueFirst = (a: LmsItemDto, b: LmsItemDto) =>
    Number(b.status === 'overdue') - Number(a.status === 'overdue') || compareItems(a, b);
  const groups: ItemGroup<StatusGroupKey>[] = [
    { key: 'pending' as const, label: STATUS_LABELS.pending, items: pending.sort(overdueFirst) },
    {
      key: 'submitted' as const,
      label: STATUS_LABELS.submitted,
      items: items.filter((i) => i.status === 'submitted').sort(compareItems),
    },
    {
      key: 'graded' as const,
      label: STATUS_LABELS.graded,
      items: items.filter((i) => i.status === 'graded').sort(compareItems),
    },
  ].filter((g) => g.items.length > 0);
  return {
    groups,
    completed: items.filter((i) => i.status === 'past_event').sort(compareItems),
  };
}
