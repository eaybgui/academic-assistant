import { useMemo } from 'react';
import type { LmsItemDto } from '../../lib/api/types';
import ItemRow from './ItemRow';
import { groupByStatus, groupByTime, type GroupedItems } from './grouping';

export type GroupBy = 'time' | 'status';

export interface ItemGroupsProps {
  items: LmsItemDto[];
  groupBy: GroupBy;
  showUniversity: boolean;
  onOpen: (item: LmsItemDto) => void;
}

/** Items grouped by time or status, with the finished ones in a collapsed section at the end. */
export default function ItemGroups({ items, groupBy, showUniversity, onOpen }: ItemGroupsProps) {
  const grouped = useMemo<GroupedItems<string>>(
    () => (groupBy === 'time' ? groupByTime(items, new Date()) : groupByStatus(items)),
    [items, groupBy],
  );
  const completedLabel = groupBy === 'time' ? 'Completed' : 'Past events';

  if (items.length === 0) {
    return (
      <p className="text-sm text-neutral-500">
        Nothing to show. Press Sync to fetch your courses&apos; items.
      </p>
    );
  }

  const rows = (list: LmsItemDto[]) => (
    <ul className="flex flex-col gap-2">
      {list.map((item) => (
        <ItemRow key={item.id} item={item} showUniversity={showUniversity} onOpen={onOpen} />
      ))}
    </ul>
  );

  return (
    <div className="flex flex-col gap-6">
      {grouped.groups.map((group) => (
        <section key={group.key} className="flex flex-col gap-2">
          <h2 className="text-sm font-semibold text-neutral-900">
            {group.label} <span className="font-normal text-neutral-500">{group.items.length}</span>
          </h2>
          {rows(group.items)}
        </section>
      ))}
      {grouped.completed.length > 0 ? (
        <details className="flex flex-col gap-2">
          <summary className="cursor-pointer text-sm font-semibold text-neutral-900">
            {completedLabel} ({grouped.completed.length})
          </summary>
          <div className="mt-2">{rows(grouped.completed)}</div>
        </details>
      ) : null}
    </div>
  );
}
