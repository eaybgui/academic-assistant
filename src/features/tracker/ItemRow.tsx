import type { LmsItemDto } from '../../lib/api/types';
import { toCommandError } from '../../lib/api/client';
import ErrorMessage from '../../components/ErrorMessage';
import { formatDateTime } from '../../components/results/format';
import NoteIndicator from './NoteIndicator';
import StatusPill from './StatusPill';
import { KIND_LABELS, canMarkDone, changeText, courseLabel } from './labels';
import {
  CHECKBOX_CLASS,
  META_ROW_CLASS,
  ROW_CLASS,
  SUBTITLE_CLASS,
  TITLE_BUTTON_CLASS,
  TITLE_ROW_CLASS,
} from './rowStyles';
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
    <li className={ROW_CLASS}>
      <div className="flex items-start gap-3">
        {canMarkDone(item) ? (
          <input
            type="checkbox"
            aria-label={`Mark ${item.title} as done`}
            checked={item.done}
            disabled={setDone.isPending}
            onChange={(e) => setDone.mutate({ id: item.id, value: e.target.checked })}
            className={CHECKBOX_CLASS}
          />
        ) : null}
        <div className="min-w-0 flex-1">
          <div className={TITLE_ROW_CLASS}>
            {showUniversity ? (
              <span className="rounded bg-neutral-100 px-1.5 py-0.5 text-xs text-neutral-600">
                {item.universityName}
              </span>
            ) : null}
            <button
              type="button"
              onClick={() => onOpen(item)}
              className={TITLE_BUTTON_CLASS}
            >
              {item.title}
            </button>
            {item.note ? <NoteIndicator /> : null}
          </div>
          <p className={SUBTITLE_CLASS}>
            {courseLabel(item)} · {KIND_LABELS[item.kind]}
          </p>
          <div className={META_ROW_CLASS}>
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
