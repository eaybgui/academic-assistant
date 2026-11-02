import { toLocalIsoDay } from '../../components/results/format';
import type { EffectiveStatus, LmsItemDto, PersonalTaskDto } from '../../lib/api/types';

/** One row of the tracker: a platform item (Moodle/Blackboard) or one of the user's personal tasks. */
export type TrackerEntry =
  | { kind: 'platform'; key: string; item: LmsItemDto }
  | { kind: 'personal'; key: string; task: PersonalTaskDto };

export type EntryStatus = EffectiveStatus | 'done';

const COLLATOR = new Intl.Collator(undefined, { sensitivity: 'base', numeric: true });

export function platformEntry(item: LmsItemDto): TrackerEntry {
  return { kind: 'platform', key: `platform-${item.id}`, item };
}

export function personalEntry(task: PersonalTaskDto): TrackerEntry {
  return { kind: 'personal', key: `personal-${task.id}`, task };
}

/** The local calendar day (`YYYY-MM-DD`) of an ISO instant. */
export function localDayOf(iso: string): string {
  return toLocalIsoDay(new Date(iso));
}

/** `today` is a local `YYYY-MM-DD`; a task dated today is still pending, whatever the hour. */
export function personalStatus(task: PersonalTaskDto, today: string): 'pending' | 'overdue' | 'done' {
  if (task.done) return 'done';
  return task.date < today ? 'overdue' : 'pending';
}

export function entryStatus(entry: TrackerEntry, today: string): EntryStatus {
  return entry.kind === 'platform' ? entry.item.status : personalStatus(entry.task, today);
}

export function entryTitle(entry: TrackerEntry): string {
  return entry.kind === 'platform' ? entry.item.title : entry.task.title;
}

/** The entry's local calendar day, or `null` when a platform item has no due date. */
export function entryDay(entry: TrackerEntry): string | null {
  if (entry.kind === 'personal') return entry.task.date;
  return entry.item.dueAt === null ? null : localDayOf(entry.item.dueAt);
}

export function isEntryCompleted(entry: TrackerEntry, today: string): boolean {
  const status = entryStatus(entry, today);
  return status === 'submitted' || status === 'graded' || status === 'past_event' || status === 'done';
}

/** Sort position in ms: a personal task counts as local midnight of its day, so it precedes that day's deadlines. */
function sortTime(entry: TrackerEntry): number | null {
  if (entry.kind === 'platform') return entry.item.dueAt === null ? null : Date.parse(entry.item.dueAt);
  const [year, month, day] = entry.task.date.split('-').map(Number);
  return new Date(year, month - 1, day).getTime();
}

/** Orders by date (undated last), then by locale-aware title, then by key. */
export function compareEntries(a: TrackerEntry, b: TrackerEntry): number {
  const timeA = sortTime(a);
  const timeB = sortTime(b);
  if (timeA !== timeB) {
    if (timeA === null) return 1;
    if (timeB === null) return -1;
    return timeA - timeB;
  }
  return COLLATOR.compare(entryTitle(a), entryTitle(b)) || COLLATOR.compare(a.key, b.key);
}
