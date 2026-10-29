import { useMemo, type ReactNode } from 'react';
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
        <details key={group.key} open className="group flex flex-col gap-2">
          <Summary>
            <h2 className="text-sm font-semibold text-neutral-900">
              {group.label} <span className="font-normal text-neutral-500">{group.items.length}</span>
            </h2>
          </Summary>
          {rows(group.items)}
        </details>
      ))}
      {grouped.completed.length > 0 ? (
        <details className="group flex flex-col gap-2">
          <Summary>
            <span className="text-sm font-semibold text-neutral-900">
              {completedLabel} ({grouped.completed.length})
            </span>
          </Summary>
          {rows(grouped.completed)}
        </details>
      ) : null}
    </div>
  );
}

/** Clickable section header with a chevron that turns when the section is open. */
function Summary({ children }: { children: ReactNode }) {
  return (
    <summary className="mb-2 flex cursor-pointer list-none items-center gap-2 [&::-webkit-details-marker]:hidden">
      <svg
        aria-hidden="true"
        viewBox="0 0 12 12"
        className="h-3 w-3 shrink-0 text-neutral-500 transition-transform group-open:rotate-90"
      >
        <path d="M4 2l4 4-4 4" fill="none" stroke="currentColor" strokeWidth="1.5" />
      </svg>
      {children}
    </summary>
  );
}
