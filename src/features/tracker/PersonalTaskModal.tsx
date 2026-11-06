import { useId, useState, type FormEvent } from 'react';
import ErrorMessage from '../../components/ErrorMessage';
import Modal from '../../components/Modal';
import ConfirmDialog from '../../components/forms/ConfirmDialog';
import Field from '../../components/forms/Field';
import Textarea from '../../components/forms/Textarea';
import TextInput from '../../components/forms/TextInput';
import { toCommandError } from '../../lib/api/client';
import type { PersonalTaskDto, PersonalTaskInput } from '../../lib/api/types';
import { usePersonalTaskMutations } from './usePersonalTaskMutations';
import { PRIMARY_BUTTON, SECONDARY_BUTTON } from '../../components/styles';

export interface PersonalTaskModalProps {
  open: boolean;
  /** `null` creates a new task. */
  task: PersonalTaskDto | null;
  /** Day (`YYYY-MM-DD`) a new task starts on. */
  initialDate: string;
  onClose: () => void;
}

/** Creates a personal task, or edits, ticks off and deletes an existing one. */
export default function PersonalTaskModal({ open, task, initialDate, onClose }: PersonalTaskModalProps) {
  return (
    <Modal open={open} title={task ? 'Edit task' : 'Add task'} onClose={onClose}>
      <TaskForm key={task?.id ?? 'new'} task={task} initialDate={initialDate} onClose={onClose} />
    </Modal>
  );
}

interface TaskFormProps {
  task: PersonalTaskDto | null;
  initialDate: string;
  onClose: () => void;
}

function TaskForm({ task, initialDate, onClose }: TaskFormProps) {
  const { create, update, setDone, remove } = usePersonalTaskMutations();
  const [title, setTitle] = useState(task?.title ?? '');
  const [date, setDate] = useState(task?.date ?? initialDate);
  const [note, setNote] = useState(task?.note ?? '');
  const [confirmingDelete, setConfirmingDelete] = useState(false);
  const ids = useId();
  const save = task ? update : create;
  const actionError = [save, setDone].find((mutation) => mutation.isError) ?? null;

  function handleSubmit(event: FormEvent) {
    event.preventDefault();
    const input: PersonalTaskInput = { title, date, note: note.trim() === '' ? null : note };
    if (task) update.mutate({ id: task.id, input }, { onSuccess: onClose });
    else create.mutate(input, { onSuccess: onClose });
  }

  function cancelDelete() {
    setConfirmingDelete(false);
    remove.reset();
  }

  return (
    <form onSubmit={handleSubmit} className="flex flex-col gap-3">
      <Field label="Title" htmlFor={`${ids}-title`}>
        <TextInput
          id={`${ids}-title`}
          required
          value={title}
          onChange={(e) => setTitle(e.target.value)}
        />
      </Field>
      <Field label="Day" htmlFor={`${ids}-date`}>
        <TextInput id={`${ids}-date`} type="date" value={date} onChange={(e) => setDate(e.target.value)} />
      </Field>
      <Field label="Note" htmlFor={`${ids}-note`}>
        <Textarea id={`${ids}-note`} rows={4} value={note} onChange={(e) => setNote(e.target.value)} />
      </Field>
      <ErrorMessage error={actionError ? toCommandError(actionError.error) : null} />
      <div className="flex flex-wrap items-center gap-2 border-t border-neutral-200 pt-3">
        <button type="submit" disabled={save.isPending} className={PRIMARY_BUTTON}>
          Save
        </button>
        {task ? (
          <>
            <button
              type="button"
              disabled={setDone.isPending}
              onClick={() => setDone.mutate({ id: task.id, value: !task.done })}
              className={SECONDARY_BUTTON}
            >
              {task.done ? 'Mark as not done' : 'Mark as done'}
            </button>
            <button type="button" onClick={() => setConfirmingDelete(true)} className={SECONDARY_BUTTON}>
              Delete
            </button>
          </>
        ) : null}
      </div>
      {task ? (
        <ConfirmDialog
          open={confirmingDelete}
          title="Delete task?"
          message="This deletes the task and its note. This cannot be undone."
          error={remove.isError ? toCommandError(remove.error) : null}
          pending={remove.isPending}
          onCancel={cancelDelete}
          onConfirm={() => remove.mutate(task.id, { onSuccess: onClose })}
        />
      ) : null}
    </form>
  );
}
