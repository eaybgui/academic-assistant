import { useMemo } from 'react';
import CollapsibleSummary from './CollapsibleSummary';
import type { TrackerEntry } from './entries';
import EntryRow from './EntryRow';
import { groupByStatus, groupByTime, type GroupedItems } from './grouping';
import { SUBSECTION_TITLE, TEXT_MUTED } from '../../components/styles';

export type GroupBy = 'time' | 'status';

export interface ItemGroupsProps {
  entries: TrackerEntry[];
  groupBy: GroupBy;
  showUniversity: boolean;
  onOpen: (entry: TrackerEntry) => void;
}

/** Items grouped by time or status, with the finished ones in a collapsed section at the end. */
export default function ItemGroups({ entries, groupBy, showUniversity, onOpen }: ItemGroupsProps) {
  const grouped = useMemo<GroupedItems<string>>(
    () => {
      const now = new Date();
      return groupBy === 'time' ? groupByTime(entries, now) : groupByStatus(entries, now);
    },
    [entries, groupBy],
  );
  const completedLabel = groupBy === 'time' ? 'Completed' : 'Past events';

  if (entries.length === 0) {
    return (
      <p className={TEXT_MUTED}>
        Nothing to show. Press Sync to fetch your courses&apos; items.
      </p>
    );
  }

  const rows = (list: TrackerEntry[]) => (
    <ul className="flex flex-col gap-2">
      {list.map((entry) => (
        <EntryRow key={entry.key} entry={entry} showUniversity={showUniversity} onOpen={onOpen} />
      ))}
    </ul>
  );

  return (
    <div className="flex flex-col gap-6">
      {grouped.groups.map((group) => (
        <details key={group.key} open className="group flex flex-col gap-2">
          <CollapsibleSummary>
            <h2 className={SUBSECTION_TITLE}>
              {group.label} <span className="font-normal text-neutral-500">{group.items.length}</span>
            </h2>
          </CollapsibleSummary>
          {rows(group.items)}
        </details>
      ))}
      {grouped.completed.length > 0 ? (
        <details className="group flex flex-col gap-2">
          <CollapsibleSummary>
            <span className={SUBSECTION_TITLE}>
              {completedLabel} ({grouped.completed.length})
            </span>
          </CollapsibleSummary>
          {rows(grouped.completed)}
        </details>
      ) : null}
    </div>
  );
}
