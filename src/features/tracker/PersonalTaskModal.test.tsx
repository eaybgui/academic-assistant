import { describe, expect, it, vi } from 'vitest';
import { render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import PersonalTaskModal from './PersonalTaskModal';
import { api } from '../../lib/api/client';
import type { PersonalTaskDto } from '../../lib/api/types';

vi.mock('../../lib/api/client', async (importOriginal) => {
  const actual = await importOriginal<typeof import('../../lib/api/client')>();
  return {
    ...actual,
    api: {
      createPersonalTask: vi.fn(),
      updatePersonalTask: vi.fn(),
      setPersonalTaskDone: vi.fn(),
      deletePersonalTask: vi.fn(),
    },
  };
});

const mockedApi = vi.mocked(api, true);

const TASK: PersonalTaskDto = {
  id: 4,
  title: 'Go to the doctor',
  date: '2026-11-05',
  note: 'Bring the referral',
  done: false,
};

function renderModal(task: PersonalTaskDto | null, onClose = vi.fn()) {
  render(
    <QueryClientProvider client={new QueryClient()}>
      <PersonalTaskModal open task={task} initialDate="2026-11-05" onClose={onClose} />
    </QueryClientProvider>,
  );
  return { onClose };
}

describe('PersonalTaskModal in create mode', () => {
  it('is titled "Add task" and defaults the day to initialDate', () => {
    renderModal(null);

    expect(screen.getByRole('heading', { name: 'Add task' })).toBeInTheDocument();
    expect(screen.getByLabelText('Day')).toHaveValue('2026-11-05');
    expect(screen.queryByRole('button', { name: 'Delete' })).not.toBeInTheDocument();
  });

  it('creates the task with the text exactly as typed and closes', async () => {
    mockedApi.createPersonalTask.mockResolvedValue(TASK);
    const { onClose } = renderModal(null);

    await userEvent.type(screen.getByLabelText('Title'), '  Go to the doctor  ');
    await userEvent.click(screen.getByRole('button', { name: 'Save' }));

    await waitFor(() => expect(onClose).toHaveBeenCalled());
    expect(mockedApi.createPersonalTask).toHaveBeenCalledWith({
      title: '  Go to the doctor  ',
      date: '2026-11-05',
      note: null,
    });
  });

  it('shows the backend message and keeps the typed text when the create is rejected', async () => {
    mockedApi.createPersonalTask.mockRejectedValue({ kind: 'validation', message: 'Enter a title' });
    const { onClose } = renderModal(null);

    await userEvent.type(screen.getByLabelText('Title'), '   ');
    await userEvent.type(screen.getByLabelText('Note'), 'Bring the referral');
    await userEvent.click(screen.getByRole('button', { name: 'Save' }));

    expect(await screen.findByRole('alert')).toHaveTextContent('Enter a title');
    expect(screen.getByLabelText('Title')).toHaveValue('   ');
    expect(screen.getByLabelText('Note')).toHaveValue('Bring the referral');
    expect(onClose).not.toHaveBeenCalled();
  });
});

describe('PersonalTaskModal in edit mode', () => {
  it('is titled "Edit task" with prefilled fields', () => {
    renderModal(TASK);

    expect(screen.getByRole('heading', { name: 'Edit task' })).toBeInTheDocument();
    expect(screen.getByLabelText('Title')).toHaveValue('Go to the doctor');
    expect(screen.getByLabelText('Day')).toHaveValue('2026-11-05');
    expect(screen.getByLabelText('Note')).toHaveValue('Bring the referral');
  });

  it('updates the task, sending a blank note as null', async () => {
    mockedApi.updatePersonalTask.mockResolvedValue(TASK);
    const { onClose } = renderModal(TASK);

    await userEvent.clear(screen.getByLabelText('Note'));
    await userEvent.type(screen.getByLabelText('Note'), '  ');
    await userEvent.click(screen.getByRole('button', { name: 'Save' }));

    await waitFor(() => expect(onClose).toHaveBeenCalled());
    expect(mockedApi.updatePersonalTask).toHaveBeenCalledWith(4, {
      title: 'Go to the doctor',
      date: '2026-11-05',
      note: null,
    });
  });

  it('toggles the done state', async () => {
    mockedApi.setPersonalTaskDone.mockResolvedValue({ ...TASK, done: true });
    renderModal(TASK);

    await userEvent.click(screen.getByRole('button', { name: 'Mark as done' }));

    expect(mockedApi.setPersonalTaskDone).toHaveBeenCalledWith(4, true);
  });

  it('offers "Mark as not done" for a done task', () => {
    renderModal({ ...TASK, done: true });

    expect(screen.getByRole('button', { name: 'Mark as not done' })).toBeInTheDocument();
  });

  it('keeps the task when the delete confirmation is cancelled', async () => {
    const { onClose } = renderModal(TASK);

    await userEvent.click(screen.getByRole('button', { name: 'Delete' }));
    const confirm = screen.getByRole('dialog', { name: 'Delete task?' });
    expect(
      within(confirm).getByText('This deletes the task and its note. This cannot be undone.'),
    ).toBeInTheDocument();
    await userEvent.click(within(confirm).getByRole('button', { name: 'Cancel' }));

    expect(mockedApi.deletePersonalTask).not.toHaveBeenCalled();
    expect(onClose).not.toHaveBeenCalled();
    expect(screen.queryByRole('dialog', { name: 'Delete task?' })).not.toBeInTheDocument();
    expect(screen.getByLabelText('Title')).toHaveValue('Go to the doctor');
  });

  it('deletes the task and closes when the deletion is confirmed', async () => {
    mockedApi.deletePersonalTask.mockResolvedValue(undefined);
    const { onClose } = renderModal(TASK);

    await userEvent.click(screen.getByRole('button', { name: 'Delete' }));
    const confirm = screen.getByRole('dialog', { name: 'Delete task?' });
    await userEvent.click(within(confirm).getByRole('button', { name: 'Delete' }));

    await waitFor(() => expect(onClose).toHaveBeenCalled());
    expect(mockedApi.deletePersonalTask).toHaveBeenCalledWith(4);
  });
});
