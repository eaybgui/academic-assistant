import { describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import ItemRow from './ItemRow';
import { api } from '../../lib/api/client';
import type { LmsItemDto } from '../../lib/api/types';

vi.mock('../../lib/api/client', async (importOriginal) => {
  const actual = await importOriginal<typeof import('../../lib/api/client')>();
  return {
    ...actual,
    api: { setItemDone: vi.fn(), dismissItemChange: vi.fn() },
  };
});

const mockedApi = vi.mocked(api, true);

function item(overrides: Partial<LmsItemDto> = {}): LmsItemDto {
  return {
    id: 7,
    universityId: 1,
    universityName: 'Universidad Demo',
    courseId: 3,
    courseName: 'Álgebra Lineal',
    courseShortName: 'ALG',
    externalId: '_10_1',
    kind: 'assignment',
    title: 'Hoja de problemas 1',
    dueAt: '2026-10-09T12:00:00Z',
    url: 'https://lms.example/item/1',
    status: 'pending',
    grade: null,
    change: null,
    removedAt: null,
    done: false,
    hidden: false,
    note: null,
    ...overrides,
  };
}

function renderRow(overrides: Partial<LmsItemDto> = {}, props: { showUniversity?: boolean; onOpen?: () => void } = {}) {
  const onOpen = props.onOpen ?? vi.fn();
  render(
    <QueryClientProvider client={new QueryClient()}>
      <ul>
        <ItemRow item={item(overrides)} showUniversity={props.showUniversity ?? false} onOpen={onOpen} />
      </ul>
    </QueryClientProvider>,
  );
  return { onOpen };
}

describe('ItemRow', () => {
  it('shows the title, course short name, kind, status and grade', () => {
    renderRow({ status: 'overdue', grade: { display: '8.5 / 10', score: 8.5, max: 10 } });

    expect(screen.getByRole('button', { name: 'Hoja de problemas 1' })).toBeInTheDocument();
    expect(screen.getByText(/ALG · Assignment/)).toBeInTheDocument();
    expect(screen.getByText('Overdue')).toBeInTheDocument();
    expect(screen.getByText('8.5 / 10')).toBeInTheDocument();
  });

  it('falls back to the course name when there is no short name', () => {
    renderRow({ courseShortName: null });
    expect(screen.getByText(/Álgebra Lineal · Assignment/)).toBeInTheDocument();
  });

  it('shows the university chip only when requested', () => {
    renderRow({}, { showUniversity: true });
    expect(screen.getByText('Universidad Demo')).toBeInTheDocument();
  });

  it('omits the university chip by default', () => {
    renderRow();
    expect(screen.queryByText('Universidad Demo')).not.toBeInTheDocument();
  });

  it('shows "No date" when there is no due date', () => {
    renderRow({ dueAt: null });
    expect(screen.getByText('No date')).toBeInTheDocument();
  });

  it('shows the change badge and dismisses it', async () => {
    const user = userEvent.setup();
    mockedApi.dismissItemChange.mockResolvedValue(item());
    renderRow({ change: { kind: 'due_changed', previousDueAt: '2026-10-05T12:00:00Z' } });

    expect(screen.getByText(/Date changed/)).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Dismiss change for Hoja de problemas 1' }));

    expect(mockedApi.dismissItemChange).toHaveBeenCalledWith(7);
  });

  it('marks the item as done from the checkbox', async () => {
    const user = userEvent.setup();
    mockedApi.setItemDone.mockResolvedValue(item({ done: true }));
    renderRow();

    await user.click(screen.getByRole('checkbox', { name: 'Mark Hoja de problemas 1 as done' }));

    expect(mockedApi.setItemDone).toHaveBeenCalledWith(7, true);
  });

  it('reflects the done state and unmarks it', async () => {
    const user = userEvent.setup();
    mockedApi.setItemDone.mockResolvedValue(item());
    renderRow({ done: true });

    const checkbox = screen.getByRole('checkbox', { name: 'Mark Hoja de problemas 1 as done' });
    expect(checkbox).toBeChecked();
    await user.click(checkbox);

    expect(mockedApi.setItemDone).toHaveBeenCalledWith(7, false);
  });

  it('has no checkbox for items already submitted or graded on the platform', () => {
    renderRow({ status: 'submitted' });
    expect(screen.queryByRole('checkbox')).not.toBeInTheDocument();
  });

  it('keeps the checkbox of a done item whose platform status is submitted', () => {
    renderRow({ status: 'submitted', done: true });
    expect(screen.getByRole('checkbox', { name: 'Mark Hoja de problemas 1 as done' })).toBeChecked();
  });

  it('shows the checkbox for an overdue item', () => {
    renderRow({ status: 'overdue' });
    expect(screen.getByRole('checkbox')).toBeInTheDocument();
  });

  it('has no checkbox for events', () => {
    renderRow({ kind: 'event' });
    expect(screen.queryByRole('checkbox')).not.toBeInTheDocument();
  });

  it('opens the details when the title is clicked', async () => {
    const user = userEvent.setup();
    const { onOpen } = renderRow();

    await user.click(screen.getByRole('button', { name: 'Hoja de problemas 1' }));

    expect(onOpen).toHaveBeenCalledWith(expect.objectContaining({ id: 7 }));
  });

  it('indicates a note and a removed item', () => {
    renderRow({ note: 'Entregar el viernes', removedAt: '2026-10-10T08:00:00Z' });

    expect(screen.getByRole('img', { name: 'Has a note' })).toBeInTheDocument();
    expect(screen.getByText('Removed from platform')).toBeInTheDocument();
  });

  it('shows a failed mutation inline', async () => {
    const user = userEvent.setup();
    mockedApi.setItemDone.mockRejectedValue({ kind: 'internal', message: 'Could not save' });
    renderRow();

    await user.click(screen.getByRole('checkbox', { name: 'Mark Hoja de problemas 1 as done' }));

    expect(await screen.findByRole('alert')).toHaveTextContent('Could not save');
  });
});
