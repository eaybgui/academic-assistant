import ErrorMessage from '../../components/ErrorMessage';
import { formatResultDate, todayLocalIso } from '../../components/results/format';
import { toCommandError } from '../../lib/api/client';
import { entryStatus, type TrackerEntry } from './entries';
import NoteIndicator from './NoteIndicator';
import {
  CHECKBOX_CLASS,
  META_ROW_CLASS,
  SUBTITLE_CLASS,
  TITLE_BUTTON_CLASS,
  TITLE_ROW_CLASS,
} from './rowStyles';
import StatusPill from './StatusPill';
import { usePersonalTaskMutations } from './usePersonalTaskMutations';
import { CARD_ROW } from '../../components/styles';

export interface PersonalTaskRowProps {
  entry: Extract<TrackerEntry, { kind: 'personal' }>;
  onOpen: (entry: TrackerEntry) => void;
}

/** One personal task: title, status, day, note indicator and the done checkbox. */
export default function PersonalTaskRow({ entry, onOpen }: PersonalTaskRowProps) {
  const { task } = entry;
  const { setDone } = usePersonalTaskMutations();

  return (
    <li className={CARD_ROW}>
      <div className="flex items-start gap-3">
        <input
          type="checkbox"
          aria-label={`Mark ${task.title} as done`}
          checked={task.done}
          disabled={setDone.isPending}
          onChange={(e) => setDone.mutate({ id: task.id, value: e.target.checked })}
          className={CHECKBOX_CLASS}
        />
        <div className="min-w-0 flex-1">
          <div className={TITLE_ROW_CLASS}>
            <button
              type="button"
              onClick={() => onOpen(entry)}
              title={task.title}
              className={TITLE_BUTTON_CLASS}
            >
              {task.title}
            </button>
            {task.note ? <NoteIndicator /> : null}
          </div>
          <p className={SUBTITLE_CLASS}>Personal</p>
          <div className={META_ROW_CLASS}>
            <StatusPill status={entryStatus(entry, todayLocalIso())} />
            <span>{formatResultDate(task.date, null)}</span>
          </div>
          <ErrorMessage error={setDone.isError ? toCommandError(setDone.error) : null} />
        </div>
      </div>
    </li>
  );
}
