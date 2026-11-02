import { describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import PersonalTaskRow from './PersonalTaskRow';
import { personalEntry } from './entries';
import { api } from '../../lib/api/client';
import type { PersonalTaskDto } from '../../lib/api/types';

vi.mock('../../lib/api/client', async (importOriginal) => {
  const actual = await importOriginal<typeof import('../../lib/api/client')>();
  return { ...actual, api: { setPersonalTaskDone: vi.fn() } };
});

const mockedApi = vi.mocked(api, true);

function task(overrides: Partial<PersonalTaskDto> = {}): PersonalTaskDto {
  return { id: 4, title: 'Go to the doctor', date: '2020-01-01', note: null, done: false, ...overrides };
}

function renderRow(overrides: Partial<PersonalTaskDto> = {}, onOpen = vi.fn()) {
  const entry = personalEntry(task(overrides));
  if (entry.kind !== 'personal') throw new Error('expected a personal entry');
  render(
    <QueryClientProvider client={new QueryClient()}>
      <ul>
        <PersonalTaskRow entry={entry} onOpen={onOpen} />
      </ul>
    </QueryClientProvider>,
  );
  return { entry, onOpen };
}

describe('PersonalTaskRow', () => {
  it('shows the title, "Personal", the day and the overdue pill for a past undone task', () => {
    renderRow();

    expect(screen.getByRole('button', { name: 'Go to the doctor' })).toBeInTheDocument();
    expect(screen.getByText('Personal')).toBeInTheDocument();
    expect(screen.getByText('1 Jan')).toBeInTheDocument();
    expect(screen.getByText('Overdue')).toBeInTheDocument();
  });

  it('shows the Done pill for a done task', () => {
    renderRow({ done: true });

    expect(screen.getByText('Done')).toBeInTheDocument();
    expect(screen.getByRole('checkbox', { name: 'Mark Go to the doctor as done' })).toBeChecked();
  });

  it('opens the entry when the title is clicked', async () => {
    const { entry, onOpen } = renderRow();

    await userEvent.click(screen.getByRole('button', { name: 'Go to the doctor' }));

    expect(onOpen).toHaveBeenCalledWith(entry);
  });

  it('marks the task as done through the checkbox', async () => {
    mockedApi.setPersonalTaskDone.mockResolvedValue(task({ done: true }));
    renderRow();

    await userEvent.click(screen.getByRole('checkbox', { name: 'Mark Go to the doctor as done' }));

    expect(mockedApi.setPersonalTaskDone).toHaveBeenCalledWith(4, true);
  });

  it('shows the note indicator only when the task has a note', () => {
    renderRow({ note: 'Bring the referral' });

    expect(screen.getByRole('img', { name: 'Has a note' })).toBeInTheDocument();
  });

  it('hides the note indicator when the task has no note', () => {
    renderRow();

    expect(screen.queryByRole('img', { name: 'Has a note' })).not.toBeInTheDocument();
  });

  it('shows a rejected done toggle inline', async () => {
    mockedApi.setPersonalTaskDone.mockRejectedValue({ kind: 'internal', message: 'Something went wrong' });
    renderRow();

    await userEvent.click(screen.getByRole('checkbox', { name: 'Mark Go to the doctor as done' }));

    expect(await screen.findByRole('alert')).toHaveTextContent('Something went wrong');
  });
});
