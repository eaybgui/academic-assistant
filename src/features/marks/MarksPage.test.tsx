import { beforeEach, describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { MemoryRouter, Route, Routes } from 'react-router';
import MarksPage from './MarksPage';
import type { LmsItemDto, LmsUniversityDto, TrackerDto } from '../../lib/api/types';

const { getTracker } = vi.hoisted(() => ({ getTracker: vi.fn() }));

vi.mock('../../lib/api/client', async (importOriginal) => {
  const actual = await importOriginal<typeof import('../../lib/api/client')>();
  return {
    ...actual,
    api: {
      getTracker,
      setItemDone: vi.fn(),
      setItemHidden: vi.fn(),
      setItemNote: vi.fn(),
      dismissItemChange: vi.fn(),
    },
  };
});
vi.mock('@tauri-apps/plugin-opener', () => ({ openUrl: vi.fn() }));

const UNIVERSITY: LmsUniversityDto = {
  id: 1,
  name: 'Universidad Demo',
  platform: 'moodle',
  baseUrl: 'https://lms.example',
  loginState: 'ok',
  accountLabel: null,
  lastSync: { at: null, outcome: null, error: null },
  courses: { tracked: 1, total: 1, unreviewed: 0 },
};

function item(overrides: Partial<LmsItemDto> = {}): LmsItemDto {
  return {
    id: 7,
    universityId: 1,
    universityName: 'Universidad Demo',
    courseId: 3,
    courseName: 'Álgebra Lineal',
    courseShortName: null,
    externalId: '_10_1',
    kind: 'assignment',
    title: 'Práctica 2',
    dueAt: '2026-03-01T10:00:00Z',
    url: null,
    status: 'graded',
    grade: { display: '8.5 / 10', score: 8.5, max: 10 },
    change: null,
    removedAt: null,
    done: false,
    hidden: false,
    note: null,
    ...overrides,
  };
}

function tracker(overrides: Partial<TrackerDto> = {}): TrackerDto {
  return { items: [], universities: [UNIVERSITY], personalTasks: [], syncRunning: false, ...overrides };
}

function renderPage() {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={queryClient}>
      <MemoryRouter initialEntries={['/marks']}>
        <Routes>
          <Route path="/marks" element={<MarksPage />} />
          <Route path="/universities" element={<p>Universities route</p>} />
        </Routes>
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

beforeEach(() => {
  getTracker.mockReset();
});

describe('MarksPage', () => {
  it('queries the tracker including hidden but not removed items', async () => {
    getTracker.mockResolvedValue(tracker());

    renderPage();

    await screen.findByRole('heading', { name: 'Marks' });
    expect(getTracker).toHaveBeenCalledWith({
      universityId: null,
      includeHidden: true,
      includeRemoved: false,
    });
  });

  it('shows each university, course and grade display', async () => {
    getTracker.mockResolvedValue(
      tracker({
        items: [
          item(),
          item({ id: 8, title: 'Examen final', grade: { display: 'Apto', score: null, max: null } }),
        ],
      }),
    );

    renderPage();

    expect(await screen.findByRole('heading', { name: 'Universidad Demo' })).toBeInTheDocument();
    expect(screen.getByText('Álgebra Lineal')).toBeInTheDocument();
    expect(screen.getByText('Práctica 2')).toBeInTheDocument();
    expect(screen.getByText('8.5 / 10')).toBeInTheDocument();
    expect(screen.getByText('Apto')).toBeInTheDocument();
  });

  it('clicking a title opens the item details', async () => {
    getTracker.mockResolvedValue(tracker({ items: [item()] }));
    renderPage();

    await userEvent.click(await screen.findByRole('button', { name: 'Práctica 2' }));

    expect(await screen.findByRole('dialog', { name: 'Práctica 2' })).toBeInTheDocument();
  });

  it('shows the empty state without marks', async () => {
    getTracker.mockResolvedValue(tracker({ items: [item({ grade: null })] }));

    renderPage();

    expect(
      await screen.findByText('No marks yet. Graded work appears here after a sync.'),
    ).toBeInTheDocument();
  });

  it('shows the add-university state with no universities', async () => {
    getTracker.mockResolvedValue(tracker({ universities: [] }));
    renderPage();

    expect(await screen.findByText('No marks yet.')).toBeInTheDocument();
    await userEvent.click(screen.getByRole('button', { name: 'Add a university' }));

    expect(await screen.findByText('Universities route')).toBeInTheDocument();
  });
});
