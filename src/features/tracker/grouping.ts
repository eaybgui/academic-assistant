import { toLocalIsoDay } from '../../components/results/format';
import {
  compareEntries,
  entryDay,
  entryStatus,
  isEntryCompleted,
  type TrackerEntry,
} from './entries';

export type TimeGroupKey = 'overdue' | 'today' | 'this_week' | 'next_week' | 'later' | 'no_date';
export type StatusGroupKey = 'pending' | 'submitted' | 'graded' | 'done' | 'past_events';
export interface ItemGroup<K extends string> {
  key: K;
  label: string;
  items: TrackerEntry[];
}
export interface GroupedItems<K extends string> {
  groups: ItemGroup<K>[];
  completed: TrackerEntry[];
}

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
  done: 'Done',
  past_events: 'Past events',
};
const STATUS_ORDER: StatusGroupKey[] = ['pending', 'submitted', 'graded', 'done'];

function addDays(d: Date, days: number): Date {
  return new Date(d.getFullYear(), d.getMonth(), d.getDate() + days);
}

function timeKey(entry: TrackerEntry, now: Date): TimeGroupKey {
  const day = entryDay(entry);
  if (day === null) return 'no_date';
  const today = toLocalIsoDay(now);
  if (entryStatus(entry, today) === 'overdue') return 'overdue';
  const start = new Date(now.getFullYear(), now.getMonth(), now.getDate());
  const daysToMonday = (8 - start.getDay()) % 7 || 7; // getDay: Sunday 0 … Saturday 6
  const nextMonday = addDays(start, daysToMonday);
  if (day < toLocalIsoDay(addDays(start, 1))) return 'today';
  if (day < toLocalIsoDay(nextMonday)) return 'this_week';
  if (day < toLocalIsoDay(addDays(nextMonday, 7))) return 'next_week';
  return 'later';
}

function collect<K extends string>(
  order: K[],
  labels: Record<K, string>,
  keyed: [K, TrackerEntry][],
): ItemGroup<K>[] {
  return order
    .map((key) => ({
      key,
      label: labels[key],
      items: keyed
        .filter(([k]) => k === key)
        .map(([, e]) => e)
        .sort(compareEntries),
    }))
    .filter((group) => group.items.length > 0);
}

export function groupByTime(entries: TrackerEntry[], now: Date): GroupedItems<TimeGroupKey> {
  const today = toLocalIsoDay(now);
  const open = entries.filter((e) => !isEntryCompleted(e, today));
  const completed = entries.filter((e) => isEntryCompleted(e, today)).sort(compareEntries);
  return {
    groups: collect(
      TIME_ORDER,
      TIME_LABELS,
      open.map((e): [TimeGroupKey, TrackerEntry] => [timeKey(e, now), e]),
    ),
    completed,
  };
}

function statusKey(entry: TrackerEntry, today: string): StatusGroupKey | null {
  const status = entryStatus(entry, today);
  if (status === 'pending' || status === 'overdue') return 'pending';
  if (status === 'past_event') return null;
  return status;
}

export function groupByStatus(entries: TrackerEntry[], now: Date): GroupedItems<StatusGroupKey> {
  const today = toLocalIsoDay(now);
  const overdueFirst = (a: TrackerEntry, b: TrackerEntry) =>
    Number(entryStatus(b, today) === 'overdue') - Number(entryStatus(a, today) === 'overdue') ||
    compareEntries(a, b);
  const keyed = entries.flatMap((e): [StatusGroupKey, TrackerEntry][] => {
    const key = statusKey(e, today);
    return key === null ? [] : [[key, e]];
  });
  const groups = collect(STATUS_ORDER, STATUS_LABELS, keyed);
  const pending = groups.find((g) => g.key === 'pending');
  pending?.items.sort(overdueFirst);
  return {
    groups,
    completed: entries.filter((e) => entryStatus(e, today) === 'past_event').sort(compareEntries),
  };
}
