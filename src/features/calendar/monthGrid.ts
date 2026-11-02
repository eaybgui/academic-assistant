import { toLocalIsoDay } from '../../components/results/format';
import { compareEntries, entryDay, entryStatus, type TrackerEntry } from '../tracker/entries';

export interface CalendarDay {
  /** Local calendar day, `YYYY-MM-DD`. */
  date: string;
  /** Whether the day belongs to the displayed month (the grid also shows its neighbours' days). */
  inMonth: boolean;
}

export type DotTone = 'overdue' | 'pending' | 'completed' | 'past' | 'personal' | 'personal_done';

/** Most dots one day cell shows; the rest collapse into "+N". */
export const MAX_DOTS = 6;

const DAYS_PER_WEEK = 7;

/** The weeks (Monday first) that cover `month` (0-11) of `year`, 4 to 6 rows of 7 days. */
export function monthGrid(year: number, month: number): CalendarDay[][] {
  const leadingDays = (new Date(year, month, 1).getDay() + 6) % DAYS_PER_WEEK;
  const daysInMonth = new Date(year, month + 1, 0).getDate();
  const weekCount = Math.ceil((leadingDays + daysInMonth) / DAYS_PER_WEEK);

  return Array.from({ length: weekCount }, (_, week) =>
    Array.from({ length: DAYS_PER_WEEK }, (_, weekday) => {
      const date = new Date(year, month, 1 - leadingDays + week * DAYS_PER_WEEK + weekday);
      return { date: toLocalIsoDay(date), inMonth: date.getMonth() === month };
    }),
  );
}

/** Groups entries by their local day, each day sorted; entries without a day are left out. */
export function entriesByDay(entries: TrackerEntry[]): Map<string, TrackerEntry[]> {
  const byDay = new Map<string, TrackerEntry[]>();
  for (const entry of entries) {
    const day = entryDay(entry);
    if (day === null) continue;
    const list = byDay.get(day);
    if (list) list.push(entry);
    else byDay.set(day, [entry]);
  }
  for (const list of byDay.values()) list.sort(compareEntries);
  return byDay;
}

/** The colour class of an entry's dot; `today` is a local `YYYY-MM-DD`. */
export function dotTone(entry: TrackerEntry, today: string): DotTone {
  const status = entryStatus(entry, today);
  if (entry.kind === 'personal') return status === 'done' ? 'personal_done' : 'personal';
  if (status === 'submitted' || status === 'graded') return 'completed';
  if (status === 'past_event') return 'past';
  return status === 'overdue' ? 'overdue' : 'pending';
}
