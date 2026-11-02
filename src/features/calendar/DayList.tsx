import { useState, type FormEvent } from 'react';
import ErrorMessage from '../../components/ErrorMessage';
import TextInput from '../../components/forms/TextInput';
import { formatLongDay } from '../../components/results/format';
import { toCommandError } from '../../lib/api/client';
import type { TrackerEntry } from '../tracker/entries';
import EntryRow from '../tracker/EntryRow';
import { usePersonalTaskMutations } from '../tracker/usePersonalTaskMutations';

export interface DayListProps {
  /** Local `YYYY-MM-DD` of the selected day. */
  day: string;
  entries: TrackerEntry[];
  onOpen: (entry: TrackerEntry) => void;
}

/** The selected day's entries, with a box to add a personal task for that day. */
export default function DayList({ day, entries, onOpen }: DayListProps) {
  const { create } = usePersonalTaskMutations();
  const [title, setTitle] = useState('');

  function handleSubmit(event: FormEvent) {
    event.preventDefault();
    create.mutate({ title, date: day, note: null }, { onSuccess: () => setTitle('') });
  }

  return (
    <section className="flex flex-col gap-3">
      <h2 className="text-sm font-semibold text-neutral-900">{formatLongDay(day)}</h2>
      {entries.length === 0 ? (
        <p className="text-sm text-neutral-500">Nothing on this day.</p>
      ) : (
        <ul className="flex flex-col gap-2">
          {entries.map((entry) => (
            <EntryRow key={entry.key} entry={entry} showUniversity onOpen={onOpen} />
          ))}
        </ul>
      )}
      <form onSubmit={handleSubmit} className="flex items-center gap-2">
        <TextInput
          aria-label="Add a task for this day"
          placeholder="Add a task for this day"
          value={title}
          onChange={(e) => setTitle(e.target.value)}
          className="flex-1"
        />
        <button
          type="submit"
          disabled={create.isPending}
          className="rounded bg-neutral-900 px-3 py-1.5 text-sm font-medium text-white hover:bg-neutral-800 disabled:opacity-50"
        >
          Add
        </button>
      </form>
      <ErrorMessage error={create.isError ? toCommandError(create.error) : null} />
    </section>
  );
}
