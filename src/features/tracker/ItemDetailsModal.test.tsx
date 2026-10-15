import { beforeEach, describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import ItemDetailsModal from './ItemDetailsModal';
import { api } from '../../lib/api/client';
import type { LmsItemDto } from '../../lib/api/types';

const { openUrl } = vi.hoisted(() => ({ openUrl: vi.fn() }));
vi.mock('@tauri-apps/plugin-opener', () => ({ openUrl }));

vi.mock('../../lib/api/client', async (importOriginal) => {
  const actual = await importOriginal<typeof import('../../lib/api/client')>();
  return {
    ...actual,
    api: { setItemDone: vi.fn(), setItemHidden: vi.fn(), setItemNote: vi.fn() },
  };
});

const mockedApi = vi.mocked(api, true);

beforeEach(() => {
  vi.resetAllMocks();
  openUrl.mockResolvedValue(undefined);
});

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

function renderModal(overrides: Partial<LmsItemDto> = {}) {
  render(
    <QueryClientProvider client={new QueryClient()}>
      <ItemDetailsModal item={item(overrides)} onClose={vi.fn()} />
    </QueryClientProvider>,
  );
}

describe('ItemDetailsModal', () => {
  it('shows the item details', () => {
    renderModal({
      status: 'graded',
      grade: { display: '8.5 / 10', score: 8.5, max: 10 },
      change: { kind: 'due_changed', previousDueAt: '2026-10-05T12:00:00Z' },
      removedAt: '2026-10-10T08:00:00Z',
    });

    expect(screen.getByRole('heading', { name: 'Hoja de problemas 1' })).toBeInTheDocument();
    expect(screen.getByText(/Universidad Demo · Álgebra Lineal · Assignment/)).toBeInTheDocument();
    expect(screen.getByText(/9 Oct 2026/)).toBeInTheDocument();
    expect(screen.getByText('Graded')).toBeInTheDocument();
    expect(screen.getByText('8.5 / 10')).toBeInTheDocument();
    expect(screen.getByText(/Date changed: 5 Oct → 9 Oct/)).toBeInTheDocument();
    expect(screen.getByText(/Removed from the platform on 10 Oct 2026/)).toBeInTheDocument();
  });

  it('renders nothing when there is no item', () => {
    render(
      <QueryClientProvider client={new QueryClient()}>
        <ItemDetailsModal item={null} onClose={vi.fn()} />
      </QueryClientProvider>,
    );
    expect(screen.queryByRole('heading')).not.toBeInTheDocument();
  });

  it('opens an https url in the system browser', async () => {
    const user = userEvent.setup();
    renderModal();

    await user.click(screen.getByRole('button', { name: 'Open in platform' }));

    expect(openUrl).toHaveBeenCalledWith('https://lms.example/item/1');
  });

  it('offers no platform link for an http url', () => {
    renderModal({ url: 'http://lms.example/item/1' });
    expect(screen.queryByRole('button', { name: 'Open in platform' })).not.toBeInTheDocument();
  });

  it('saves a blank note as null', async () => {
    const user = userEvent.setup();
    mockedApi.setItemNote.mockResolvedValue(item());
    renderModal({ note: 'Antigua' });

    const textarea = screen.getByLabelText('Note');
    expect(textarea).toHaveValue('Antigua');
    await user.clear(textarea);
    await user.type(textarea, '   ');
    await user.click(screen.getByRole('button', { name: 'Save note' }));

    expect(mockedApi.setItemNote).toHaveBeenCalledWith(7, null);
  });

  it('saves a non-blank note as typed', async () => {
    const user = userEvent.setup();
    mockedApi.setItemNote.mockResolvedValue(item());
    renderModal();

    await user.type(screen.getByLabelText('Note'), 'Entregar el viernes');
    await user.click(screen.getByRole('button', { name: 'Save note' }));

    expect(mockedApi.setItemNote).toHaveBeenCalledWith(7, 'Entregar el viernes');
  });

  it('shows a rejected note inline and keeps the text', async () => {
    const user = userEvent.setup();
    mockedApi.setItemNote.mockRejectedValue({
      kind: 'validation',
      message: 'Notes can have at most 2000 characters',
    });
    renderModal();

    await user.type(screen.getByLabelText('Note'), 'demasiado largo');
    await user.click(screen.getByRole('button', { name: 'Save note' }));

    expect(await screen.findByText('Notes can have at most 2000 characters')).toBeInTheDocument();
    expect(screen.getByLabelText('Note')).toHaveValue('demasiado largo');
  });

  it('hides the item', async () => {
    const user = userEvent.setup();
    mockedApi.setItemHidden.mockResolvedValue(item({ hidden: true }));
    renderModal();

    await user.click(screen.getByRole('button', { name: 'Hide' }));

    expect(mockedApi.setItemHidden).toHaveBeenCalledWith(7, true);
  });

  it('shows a hidden item again', async () => {
    const user = userEvent.setup();
    mockedApi.setItemHidden.mockResolvedValue(item());
    renderModal({ hidden: true });

    await user.click(screen.getByRole('button', { name: 'Show again' }));

    expect(mockedApi.setItemHidden).toHaveBeenCalledWith(7, false);
  });

  it('toggles done', async () => {
    const user = userEvent.setup();
    mockedApi.setItemDone.mockResolvedValue(item({ done: true }));
    renderModal();

    await user.click(screen.getByRole('button', { name: 'Mark as done' }));

    expect(mockedApi.setItemDone).toHaveBeenCalledWith(7, true);
  });

  it('offers "Mark as not done" for a done item', () => {
    renderModal({ done: true });
    expect(screen.getByRole('button', { name: 'Mark as not done' })).toBeInTheDocument();
  });

  it('has no done toggle for events', () => {
    renderModal({ kind: 'event' });
    expect(screen.queryByRole('button', { name: /Mark as/ })).not.toBeInTheDocument();
  });
});
