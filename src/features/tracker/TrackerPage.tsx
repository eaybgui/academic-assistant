import { useEffect, useState } from 'react';
import { useNavigate } from 'react-router';
import { keepPreviousData, useQuery } from '@tanstack/react-query';
import ErrorMessage from '../../components/ErrorMessage';
import { formatLastSynced } from '../../components/results/format';
import { api, toCommandError } from '../../lib/api/client';
import { queryKeys } from '../../lib/api/queryKeys';
import type { LmsUniversityDto, TrackerQuery } from '../../lib/api/types';
import { useSync } from '../sync/useSync';
import ItemDetailsModal from './ItemDetailsModal';
import ItemGroups, { type GroupBy } from './ItemGroups';
import SyncBanners from './SyncBanners';
import { usePersistentState } from './usePersistentState';

const TAB_KEY = 'academic-assistant.tracker-tab';
const GROUP_KEY = 'academic-assistant.tracker-group';
const GROUP_OPTIONS: readonly GroupBy[] = ['time', 'status'];
const GROUP_LABELS: Record<GroupBy, string> = {
  time: 'Time',
  status: 'Status',
};

const CARD = 'rounded-lg border border-neutral-200 bg-white p-6';
const PRIMARY_BUTTON =
  'rounded bg-neutral-900 px-3 py-1.5 text-sm font-medium text-white hover:bg-neutral-800 disabled:opacity-50';

function isTabValue(value: string): value is string {
  return value === 'all' || /^\d+$/.test(value);
}

function latestSyncAt(universities: LmsUniversityDto[]): string | null {
  const times = universities.flatMap((u) => (u.lastSync.at ? [u.lastSync.at] : []));
  return times.length > 0 ? times.reduce((a, b) => (Date.parse(a) >= Date.parse(b) ? a : b)) : null;
}

function segmentClass(active: boolean): string {
  return `px-3 py-1 text-sm ${active ? 'bg-neutral-900 text-white' : 'bg-white text-neutral-700 hover:bg-neutral-50'}`;
}

/** Main screen: deliverables and course events from every connected university, grouped by time or status. */
export default function TrackerPage() {
  const navigate = useNavigate();
  const { running, progress, error: syncError, startSync } = useSync();
  const [tab, setTab] = usePersistentState<string>(TAB_KEY, 'all', isTabValue);
  const [groupBy, setGroupBy] = usePersistentState<GroupBy>(GROUP_KEY, 'time', GROUP_OPTIONS);
  const [includeHidden, setIncludeHidden] = useState(false);
  const [includeRemoved, setIncludeRemoved] = useState(false);
  const [openItemId, setOpenItemId] = useState<number | null>(null);

  const query: TrackerQuery = {
    universityId: tab === 'all' ? null : Number(tab),
    includeHidden,
    includeRemoved,
  };
  const { data, error, isPlaceholderData } = useQuery({
    queryKey: queryKeys.tracker(query),
    queryFn: () => api.getTracker(query),
    placeholderData: keepPreviousData,
  });

  const tabMissing =
    data !== undefined && tab !== 'all' && !data.universities.some((u) => String(u.id) === tab);
  useEffect(() => {
    if (tabMissing) setTab('all');
  }, [tabMissing, setTab]);

  // Forget the open item once fresh data no longer has it, so it can't pop open again if it returns.
  const openItemGone =
    openItemId !== null &&
    data !== undefined &&
    !isPlaceholderData &&
    !data.items.some((i) => i.id === openItemId);
  if (openItemGone) setOpenItemId(null);

  if (error && !data) return <ErrorMessage error={toCommandError(error)} />;
  if (!data) return <p className="text-sm text-neutral-500">Loading…</p>;

  const { items, universities, syncRunning } = data;

  if (universities.length === 0) {
    return (
      <div className={`${CARD} flex max-w-xl flex-col items-start gap-3`}>
        <h1 className="text-lg font-semibold text-neutral-900">Track your deliverables</h1>
        <p className="text-sm text-neutral-600">
          Add your university to see assignments, exams and course events in one place.
        </p>
        <button
          type="button"
          onClick={() => void navigate('/universities', { state: { openAdd: true } })}
          className={PRIMARY_BUTTON}
        >
          Add your first university
        </button>
      </div>
    );
  }

  if (universities.every((u) => u.courses.tracked === 0)) {
    return (
      <div className={`${CARD} flex max-w-xl flex-col items-start gap-3`}>
        <h1 className="text-2xl font-semibold text-neutral-900">Dashboard</h1>
        <p className="text-sm text-neutral-600">No courses are tracked yet.</p>
        <button
          type="button"
          onClick={() => void navigate('/universities')}
          className={PRIMARY_BUTTON}
        >
          Choose courses
        </button>
      </div>
    );
  }

  const shown = tab === 'all' ? universities : universities.filter((u) => String(u.id) === tab);
  const selectedUntracked = tab !== 'all' && shown[0]?.courses.tracked === 0 ? shown[0] : null;
  const lastSyncAt = latestSyncAt(shown);
  const syncBusy = running || syncRunning;
  const openItem = items.find((i) => i.id === openItemId) ?? null;

  return (
    <div className="flex flex-col gap-4">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <h1 className="text-2xl font-semibold text-neutral-900">Dashboard</h1>
        <div className="flex flex-col items-end gap-1">
          <div className="flex items-center gap-3">
            {lastSyncAt ? (
              <span className="text-xs text-neutral-500">
                Last synced {formatLastSynced(lastSyncAt)}
              </span>
            ) : null}
            <button
              type="button"
              disabled={syncBusy}
              onClick={() => void startSync(tab === 'all' ? null : Number(tab))}
              className={PRIMARY_BUTTON}
            >
              {syncBusy ? (progress ?? 'Syncing…') : 'Sync'}
            </button>
          </div>
          <ErrorMessage error={syncError} />
        </div>
      </div>

      <div className="flex flex-wrap items-center gap-4">
        <div role="tablist" aria-label="University" className="flex flex-wrap gap-1">
          {[
            { value: 'all', label: 'All' },
            ...universities.map((u) => ({
              value: String(u.id),
              label: u.name,
            })),
          ].map(({ value, label }) => (
            <button
              key={value}
              type="button"
              role="tab"
              aria-selected={tab === value}
              onClick={() => setTab(value)}
              className={`rounded px-3 py-1 text-sm ${
                tab === value
                  ? 'bg-neutral-900 text-white'
                  : 'text-neutral-700 hover:bg-neutral-100'
              }`}
            >
              {label}
            </button>
          ))}
        </div>
        <div
          role="group"
          aria-label="Group by"
          className="flex items-center gap-2 text-sm text-neutral-600"
        >
          <span>Group by</span>
          <div className="flex overflow-hidden rounded border border-neutral-300">
            {GROUP_OPTIONS.map((option) => (
              <button
                key={option}
                type="button"
                aria-pressed={groupBy === option}
                onClick={() => setGroupBy(option)}
                className={segmentClass(groupBy === option)}
              >
                {GROUP_LABELS[option]}
              </button>
            ))}
          </div>
        </div>
        <label className="flex items-center gap-1.5 text-sm text-neutral-700">
          <input
            type="checkbox"
            checked={includeHidden}
            onChange={(e) => setIncludeHidden(e.target.checked)}
          />
          Show hidden
        </label>
        <label className="flex items-center gap-1.5 text-sm text-neutral-700">
          <input
            type="checkbox"
            checked={includeRemoved}
            onChange={(e) => setIncludeRemoved(e.target.checked)}
          />
          Show removed
        </label>
      </div>

      <SyncBanners universities={shown} />
      {items.length === 0 && selectedUntracked ? (
        <div className="flex flex-col items-start gap-2">
          <p className="text-sm text-neutral-500">
            No courses are tracked for {selectedUntracked.name}.
          </p>
          <button
            type="button"
            onClick={() => void navigate('/universities')}
            className={PRIMARY_BUTTON}
          >
            Choose courses
          </button>
        </div>
      ) : (
        <ItemGroups
          items={items}
          groupBy={groupBy}
          showUniversity={tab === 'all'}
          onOpen={(item) => setOpenItemId(item.id)}
        />
      )}
      <ItemDetailsModal item={openItem} onClose={() => setOpenItemId(null)} />
    </div>
  );
}
