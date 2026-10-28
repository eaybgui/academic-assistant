import type { LmsItemDto } from '../../lib/api/types';
import { toCommandError } from '../../lib/api/client';
import ErrorMessage from '../../components/ErrorMessage';
import { formatDateTime } from '../../components/results/format';
import StatusPill from './StatusPill';
import { KIND_LABELS, canMarkDone, changeText, courseLabel } from './labels';
import { useItemMutations } from './useItemMutations';

export interface ItemRowProps {
  item: LmsItemDto;
  showUniversity: boolean;
  onOpen: (item: LmsItemDto) => void;
}

const CHIP = 'rounded-full px-2 py-0.5 font-medium';

/** One tracker item: title, course/kind, status, grade, due date, unseen-change badge and the done checkbox. */
export default function ItemRow({ item, showUniversity, onOpen }: ItemRowProps) {
  const { setDone, setHidden, dismissChange } = useItemMutations();
  const failed = [setDone, setHidden, dismissChange].find((mutation) => mutation.isError) ?? null;

  return (
    <li className="rounded-lg border border-neutral-200 bg-white px-4 py-3">
      <div className="flex items-start gap-3">
        {canMarkDone(item) ? (
          <input
            type="checkbox"
            aria-label={`Mark ${item.title} as done`}
            checked={item.done}
            disabled={setDone.isPending}
            onChange={(e) => setDone.mutate({ id: item.id, value: e.target.checked })}
            className="mt-1 h-4 w-4"
          />
        ) : null}
        <div className="min-w-0 flex-1">
          <div className="flex flex-wrap items-center gap-2">
            {showUniversity ? (
              <span className="rounded bg-neutral-100 px-1.5 py-0.5 text-xs text-neutral-600">
                {item.universityName}
              </span>
            ) : null}
            <button
              type="button"
              onClick={() => onOpen(item)}
              className="truncate text-left text-sm font-semibold text-neutral-900 hover:underline"
            >
              {item.title}
            </button>
            {item.note ? (
              <span role="img" aria-label="Has a note" title="Has a note" className="text-xs text-neutral-500">
                &#9998;
              </span>
            ) : null}
          </div>
          <p className="mt-0.5 text-xs text-neutral-500">
            {courseLabel(item)} · {KIND_LABELS[item.kind]}
          </p>
          <div className="mt-1.5 flex flex-wrap items-center gap-2 text-xs text-neutral-600">
            <StatusPill status={item.status} />
            {item.grade ? <span className="font-medium text-neutral-900">{item.grade.display}</span> : null}
            <span>{item.dueAt ? formatDateTime(item.dueAt) : 'No date'}</span>
            {item.change ? (
              <span className={`inline-flex items-center gap-1 ${CHIP} bg-amber-100 text-amber-800`}>
                {changeText(item.change, item.dueAt)}
                <button
                  type="button"
                  aria-label={`Dismiss change for ${item.title}`}
                  disabled={dismissChange.isPending}
                  onClick={() => dismissChange.mutate(item.id)}
                  className="hover:text-amber-950"
                >
                  &times;
                </button>
              </span>
            ) : null}
            {item.hidden ? (
              <span className={`inline-flex items-center gap-1 ${CHIP} bg-neutral-200 text-neutral-700`}>
                Hidden
                <button
                  type="button"
                  aria-label={`Show ${item.title} again`}
                  title="Show again"
                  disabled={setHidden.isPending}
                  onClick={() => setHidden.mutate({ id: item.id, value: false })}
                  className="hover:text-neutral-900"
                >
                  &times;
                </button>
              </span>
            ) : null}
            {item.removedAt ? (
              <span className={`${CHIP} bg-red-100 text-red-800`}>Removed from platform</span>
            ) : null}
          </div>
          <ErrorMessage error={failed ? toCommandError(failed.error) : null} />
        </div>
      </div>
    </li>
  );
}
