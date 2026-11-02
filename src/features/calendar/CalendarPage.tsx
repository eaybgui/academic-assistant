import { useMemo, useState } from 'react';
import { useQuery } from '@tanstack/react-query';
import ErrorMessage from '../../components/ErrorMessage';
import { formatMonthYear, todayLocalIso } from '../../components/results/format';
import { api, toCommandError } from '../../lib/api/client';
import { queryKeys } from '../../lib/api/queryKeys';
import type { TrackerQuery } from '../../lib/api/types';
import EntryDetailsModal from '../tracker/EntryDetailsModal';
import { personalEntry, platformEntry, type TrackerEntry } from '../tracker/entries';
import { useOpenEntry } from '../tracker/useOpenEntry';
import CalendarGrid from './CalendarGrid';
import DayList from './DayList';
import { entriesByDay, monthGrid } from './monthGrid';

/** Same query as the dashboard's All tab, so both screens share one cache entry. */
const ALL: TrackerQuery = { universityId: null, includeHidden: false, includeRemoved: false };

const NAV_BUTTON =
  'rounded border border-neutral-300 px-3 py-1.5 text-sm text-neutral-700 hover:bg-neutral-50';

interface ViewedMonth {
  year: number;
  month: number;
}

function monthOf(isoDay: string): ViewedMonth {
  const [year, month] = isoDay.split('-').map(Number);
  return { year, month: month - 1 };
}

/** Shifts a month by `delta` months; `new Date` carries month overflow into the year. */
function shiftMonth({ year, month }: ViewedMonth, delta: number): ViewedMonth {
  const shifted = new Date(year, month + delta, 1);
  return { year: shifted.getFullYear(), month: shifted.getMonth() };
}

/** Month view of every dated entry (platform items and personal tasks), with the selected day listed underneath. */
export default function CalendarPage() {
  const today = todayLocalIso();
  const [viewed, setViewed] = useState<ViewedMonth>(() => monthOf(today));
  const [selected, setSelected] = useState(today);

  const { data, error } = useQuery({
    queryKey: queryKeys.tracker(ALL),
    queryFn: () => api.getTracker(ALL),
  });

  const entries = useMemo<TrackerEntry[] | undefined>(
    () => data && [...data.items.map(platformEntry), ...data.personalTasks.map(personalEntry)],
    [data],
  );
  const byDay = useMemo(() => entriesByDay(entries ?? []), [entries]);
  const weeks = useMemo(() => monthGrid(viewed.year, viewed.month), [viewed]);
  const { openEntry, open, close } = useOpenEntry(entries, false);

  if (error && !data) return <ErrorMessage error={toCommandError(error)} />;
  if (!data) return <p className="text-sm text-neutral-500">Loading…</p>;

  function goToToday() {
    setViewed(monthOf(today));
    setSelected(today);
  }

  return (
    <div className="flex max-w-3xl flex-col gap-4">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <h1 className="text-2xl font-semibold text-neutral-900">Calendar</h1>
        <div className="flex items-center gap-2">
          <button
            type="button"
            aria-label="Previous month"
            onClick={() => setViewed(shiftMonth(viewed, -1))}
            className={NAV_BUTTON}
          >
            ‹
          </button>
          <span className="min-w-36 text-center text-sm font-medium text-neutral-900">
            {formatMonthYear(viewed.year, viewed.month)}
          </span>
          <button
            type="button"
            aria-label="Next month"
            onClick={() => setViewed(shiftMonth(viewed, 1))}
            className={NAV_BUTTON}
          >
            ›
          </button>
          <button type="button" onClick={goToToday} className={NAV_BUTTON}>
            Today
          </button>
        </div>
      </div>
      <CalendarGrid
        weeks={weeks}
        entriesByDay={byDay}
        selected={selected}
        today={today}
        onSelect={setSelected}
      />
      <DayList day={selected} entries={byDay.get(selected) ?? []} onOpen={open} />
      <EntryDetailsModal entry={openEntry} onClose={close} />
    </div>
  );
}
