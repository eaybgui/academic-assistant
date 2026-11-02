import { formatLongDay } from '../../components/results/format';
import { entryTitle, type TrackerEntry } from '../tracker/entries';
import { dotTone, MAX_DOTS, type CalendarDay, type DotTone } from './monthGrid';

const WEEKDAYS = ['Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat', 'Sun'] as const;

const DOT_CLASS: Record<DotTone, string> = {
  overdue: 'bg-red-600',
  pending: 'bg-blue-600',
  completed: 'bg-green-600',
  past: 'bg-neutral-400',
  personal: 'bg-violet-600',
  personal_done: 'bg-violet-300',
};

const LEGEND: readonly { tone: DotTone; label: string }[] = [
  { tone: 'overdue', label: 'Overdue' },
  { tone: 'pending', label: 'Pending' },
  { tone: 'completed', label: 'Submitted / graded' },
  { tone: 'past', label: 'Past event' },
  { tone: 'personal', label: 'Personal' },
];

export interface CalendarGridProps {
  weeks: CalendarDay[][];
  entriesByDay: Map<string, TrackerEntry[]>;
  /** Local `YYYY-MM-DD` of the selected day. */
  selected: string;
  /** Local `YYYY-MM-DD` of today. */
  today: string;
  onSelect: (day: string) => void;
}

function Dot({ tone, title }: { tone: DotTone; title?: string }) {
  return (
    <span
      title={title}
      aria-label={title}
      className={`h-2 w-2 shrink-0 rounded-full ${DOT_CLASS[tone]}`}
    />
  );
}

interface DayCellProps {
  day: CalendarDay;
  entries: TrackerEntry[];
  selected: boolean;
  today: string;
  onSelect: (day: string) => void;
}

function DayCell({ day, entries, selected, today, onSelect }: DayCellProps) {
  const isToday = day.date === today;
  const hidden = entries.length - MAX_DOTS;
  const border = selected ? 'border-neutral-900' : isToday ? 'border-neutral-400' : 'border-transparent';

  return (
    <button
      type="button"
      aria-label={formatLongDay(day.date)}
      aria-pressed={selected}
      aria-current={isToday ? 'date' : undefined}
      onClick={() => onSelect(day.date)}
      className={`flex h-16 flex-col items-start gap-1 rounded border p-1.5 text-left hover:bg-neutral-50 ${border} ${
        selected ? 'bg-neutral-100' : 'bg-white'
      } ${day.inMonth ? 'text-neutral-900' : 'text-neutral-400'}`}
    >
      <span className={`text-xs ${isToday ? 'font-semibold' : ''}`}>{Number(day.date.slice(8))}</span>
      <span className="flex flex-wrap items-center gap-0.5">
        {entries.slice(0, MAX_DOTS).map((entry) => (
          <Dot key={entry.key} tone={dotTone(entry, today)} title={entryTitle(entry)} />
        ))}
        {hidden > 0 ? <span className="text-[10px] leading-none text-neutral-500">+{hidden}</span> : null}
      </span>
    </button>
  );
}

/** A Monday-first month grid: one button per day with a coloured dot for each of its entries, then the colour legend. */
export default function CalendarGrid({ weeks, entriesByDay, selected, today, onSelect }: CalendarGridProps) {
  return (
    <div className="flex flex-col gap-2">
      <div className="grid grid-cols-7 gap-1">
        {WEEKDAYS.map((weekday) => (
          <span key={weekday} className="px-1.5 text-xs font-medium text-neutral-500">
            {weekday}
          </span>
        ))}
        {weeks.flat().map((day) => (
          <DayCell
            key={day.date}
            day={day}
            entries={entriesByDay.get(day.date) ?? []}
            selected={day.date === selected}
            today={today}
            onSelect={onSelect}
          />
        ))}
      </div>
      <ul className="flex flex-wrap gap-x-4 gap-y-1 text-xs text-neutral-600">
        {LEGEND.map(({ tone, label }) => (
          <li key={tone} className="flex items-center gap-1.5">
            <Dot tone={tone} />
            {label}
          </li>
        ))}
      </ul>
    </div>
  );
}
