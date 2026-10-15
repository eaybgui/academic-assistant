import { beforeEach, describe, expect, it, vi } from 'vitest';
import { render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { MemoryRouter, Route, Routes } from 'react-router';
import TrackerPage from './TrackerPage';
import type { SyncState } from '../sync/useSync';
import type { LmsItemDto, LmsUniversityDto, TrackerDto } from '../../lib/api/types';

const { getTracker, lmsSignIn, dismissAllChanges, sync } = vi.hoisted(() => ({
  getTracker: vi.fn(),
  lmsSignIn: vi.fn(),
  dismissAllChanges: vi.fn(),
  sync: {} as { state: SyncState },
}));

vi.mock('../../lib/api/client', async (importOriginal) => {
  const actual = await importOriginal<typeof import('../../lib/api/client')>();
  return {
    ...actual,
    api: {
      getTracker,
      lmsSignIn,
      dismissAllChanges,
      setItemDone: vi.fn(),
      setItemHidden: vi.fn(),
      setItemNote: vi.fn(),
      dismissItemChange: vi.fn(),
    },
  };
});
vi.mock('../sync/useSync', () => ({ useSync: () => sync.state }));
vi.mock('@tauri-apps/plugin-opener', () => ({ openUrl: vi.fn() }));

const startSync = vi.fn();
const clearResults = vi.fn();

function university(overrides: Partial<LmsUniversityDto> = {}): LmsUniversityDto {
  return {
    id: 1,
    name: 'UV',
    platform: 'moodle',
    baseUrl: 'https://lms.example',
    loginState: 'ok',
    accountLabel: null,
    lastSync: { at: null, outcome: null, error: null },
    courses: { tracked: 2, total: 3, unreviewed: 0 },
    ...overrides,
  };
}

const UV = university();
const UAH = university({ id: 2, name: 'UAH' });

function item(overrides: Partial<LmsItemDto> = {}): LmsItemDto {
  return {
    id: 7,
    universityId: 1,
    universityName: 'UV',
    courseId: 3,
    courseName: 'Álgebra Lineal',
    courseShortName: null,
    externalId: '_10_1',
    kind: 'assignment',
    title: 'Hoja de problemas 1',
    dueAt: null,
    url: null,
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

function daysFromNow(days: number): string {
  return new Date(Date.now() + days * 86_400_000).toISOString();
}

function tracker(overrides: Partial<TrackerDto> = {}): TrackerDto {
  return {
    items: [],
    universities: [UV, UAH],
    syncRunning: false,
    ...overrides,
  };
}

function setSync(overrides: Partial<SyncState> = {}) {
  sync.state = {
    running: false,
    progress: null,
    lastResults: null,
    error: null,
    startSync,
    clearResults,
    ...overrides,
  };
}

function renderPage() {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  return render(
    <QueryClientProvider client={client}>
      <MemoryRouter initialEntries={['/']}>
        <Routes>
          <Route path="/" element={<TrackerPage />} />
          <Route path="/universities" element={<p>Universities route</p>} />
        </Routes>
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

beforeEach(() => {
  vi.resetAllMocks();
  localStorage.clear();
  setSync();
  getTracker.mockResolvedValue(tracker());
  startSync.mockResolvedValue(undefined);
});

describe('TrackerPage', () => {
  it('shows the first-run card when there are no universities and navigates to add one', async () => {
    getTracker.mockResolvedValue(tracker({ universities: [] }));
    renderPage();
    expect(
      await screen.findByRole('heading', { name: 'Track your deliverables' }),
    ).toBeInTheDocument();
    expect(
      screen.getByText(
        'Add your university to see assignments, exams and course events in one place.',
      ),
    ).toBeInTheDocument();
    await userEvent.click(screen.getByRole('button', { name: 'Add your first university' }));
    expect(await screen.findByText('Universities route')).toBeInTheDocument();
  });

  it('shows an empty state when no course is tracked', async () => {
    getTracker.mockResolvedValue(
      tracker({
        universities: [university({ courses: { tracked: 0, total: 4, unreviewed: 0 } })],
      }),
    );
    renderPage();
    expect(await screen.findByText('No courses are tracked yet.')).toBeInTheDocument();
    await userEvent.click(screen.getByRole('button', { name: 'Choose courses' }));
    expect(await screen.findByText('Universities route')).toBeInTheDocument();
  });

  it('groups items by time and collects completed ones', async () => {
    getTracker.mockResolvedValue(
      tracker({
        items: [
          item({
            id: 1,
            title: 'Late essay',
            status: 'overdue',
            dueAt: daysFromNow(-2),
          }),
          item({ id: 2, title: 'Final project', dueAt: daysFromNow(30) }),
          item({
            id: 3,
            title: 'Quiz 1',
            status: 'graded',
            dueAt: daysFromNow(-5),
          }),
        ],
      }),
    );
    renderPage();
    const overdue = (await screen.findByRole('heading', { name: /Overdue/ })).closest('section')!;
    expect(within(overdue).getByText('Late essay')).toBeInTheDocument();
    const later = screen.getByRole('heading', { name: /Later/ }).closest('section')!;
    expect(within(later).getByText('Final project')).toBeInTheDocument();
    expect(screen.getByText('Completed (1)')).toBeInTheDocument();
  });

  it('groups items by status when selected', async () => {
    getTracker.mockResolvedValue(
      tracker({
        items: [
          item({ id: 1, title: 'Final project', dueAt: daysFromNow(30) }),
          item({
            id: 3,
            title: 'Quiz 1',
            status: 'graded',
            dueAt: daysFromNow(-5),
          }),
        ],
      }),
    );
    renderPage();
    await userEvent.click(await screen.findByRole('button', { name: 'Status' }));
    expect(screen.getByRole('heading', { name: /Pending/ })).toBeInTheDocument();
    expect(screen.getByRole('heading', { name: /Graded/ })).toBeInTheDocument();
    expect(localStorage.getItem('academic-assistant.tracker-group')).toBe('status');
  });

  it('shows a hint when there is nothing to show', async () => {
    renderPage();
    expect(
      await screen.findByText("Nothing to show. Press Sync to fetch your courses' items."),
    ).toBeInTheDocument();
  });

  it('offers to choose courses on the tab of a university with no tracked courses', async () => {
    getTracker.mockResolvedValue(
      tracker({
        universities: [UV, university({ id: 2, name: 'UAH', courses: { tracked: 0, total: 3, unreviewed: 0 } })],
      }),
    );
    renderPage();
    await userEvent.click(await screen.findByRole('tab', { name: 'UAH' }));
    expect(await screen.findByText('No courses are tracked for UAH.')).toBeInTheDocument();
    expect(screen.queryByText(/Press Sync/)).not.toBeInTheDocument();
    await userEvent.click(screen.getByRole('button', { name: 'Choose courses' }));
    expect(await screen.findByText('Universities route')).toBeInTheDocument();
  });

  it('switches university tab, persists it and restores it', async () => {
    const first = renderPage();
    await userEvent.click(await screen.findByRole('tab', { name: 'UAH' }));
    await waitFor(() =>
      expect(getTracker).toHaveBeenLastCalledWith({
        universityId: 2,
        includeHidden: false,
        includeRemoved: false,
      }),
    );
    expect(localStorage.getItem('academic-assistant.tracker-tab')).toBe('2');
    first.unmount();

    renderPage();
    expect(await screen.findByRole('tab', { name: 'UAH', selected: true })).toBeInTheDocument();
  });

  it('falls back to All when the stored university no longer exists', async () => {
    localStorage.setItem('academic-assistant.tracker-tab', '99');
    renderPage();
    expect(await screen.findByRole('tab', { name: 'All', selected: true })).toBeInTheDocument();
    await waitFor(() =>
      expect(getTracker).toHaveBeenLastCalledWith({
        universityId: null,
        includeHidden: false,
        includeRemoved: false,
      }),
    );
  });

  it('requests hidden items when "Show hidden" is checked', async () => {
    renderPage();
    await userEvent.click(await screen.findByRole('checkbox', { name: 'Show hidden' }));
    await waitFor(() =>
      expect(getTracker).toHaveBeenLastCalledWith({
        universityId: null,
        includeHidden: true,
        includeRemoved: false,
      }),
    );
  });

  it('syncs everything from the All tab and one university from its tab', async () => {
    renderPage();
    await userEvent.click(await screen.findByRole('button', { name: 'Sync' }));
    expect(startSync).toHaveBeenLastCalledWith(null);
    await userEvent.click(screen.getByRole('tab', { name: 'UAH' }));
    await userEvent.click(screen.getByRole('button', { name: 'Sync' }));
    expect(startSync).toHaveBeenLastCalledWith(2);
  });

  it('disables Sync and shows the progress while a sync runs', async () => {
    setSync({ running: true, progress: 'UV: Fetching items' });
    renderPage();
    expect(await screen.findByRole('button', { name: 'UV: Fetching items' })).toBeDisabled();
  });

  it('disables Sync while the backend reports a running sync', async () => {
    getTracker.mockResolvedValue(tracker({ syncRunning: true }));
    renderPage();
    expect(await screen.findByRole('button', { name: /Syncing/ })).toBeDisabled();
  });

  it('shows a sync start error next to the button', async () => {
    setSync({
      error: { kind: 'conflict', message: 'A sync is already running' },
    });
    renderPage();
    expect(await screen.findByText('A sync is already running')).toBeInTheDocument();
  });

  it('uses the same heading size as the other pages', async () => {
    renderPage();
    expect(await screen.findByRole('heading', { name: 'Dashboard' })).toHaveClass('text-2xl');
  });

  it('shows when the shown universities were last synced', async () => {
    getTracker.mockResolvedValue(
      tracker({
        universities: [
          university({
            lastSync: { at: daysFromNow(-3), outcome: 'success', error: null },
          }),
          university({
            id: 2,
            name: 'UAH',
            lastSync: {
              at: new Date().toISOString(),
              outcome: 'success',
              error: null,
            },
          }),
        ],
      }),
    );
    renderPage();
    expect(await screen.findByText(/^Last synced today at \d{2}:\d{2}$/)).toBeInTheDocument();
  });

  it('asks to sign in again and syncs after signing in', async () => {
    getTracker.mockResolvedValue(
      tracker({ universities: [university({ loginState: 'expired' }), UAH] }),
    );
    lmsSignIn.mockResolvedValue(university());
    renderPage();
    const banner = (await screen.findByText('UV needs you to sign in again')).closest(
      '[role="alert"]',
    )!;
    await userEvent.click(within(banner as HTMLElement).getByRole('button', { name: 'Sign in' }));
    await waitFor(() => expect(startSync).toHaveBeenCalledWith(1));
    expect(lmsSignIn).toHaveBeenCalledWith(1);
  });

  it('shows no sign-in banner once the connection works again after a login_required sync', async () => {
    getTracker.mockResolvedValue(
      tracker({
        universities: [
          university({
            loginState: 'ok',
            lastSync: { at: daysFromNow(-1), outcome: 'login_required', error: null },
          }),
        ],
      }),
    );
    renderPage();
    await screen.findByRole('heading', { name: 'Dashboard' });
    expect(screen.queryByText('UV needs you to sign in again')).not.toBeInTheDocument();
  });

  it('asks to sign in when the last sync needed it and the connection is not confirmed', async () => {
    getTracker.mockResolvedValue(
      tracker({
        universities: [
          university({
            loginState: 'never',
            lastSync: { at: daysFromNow(-1), outcome: 'login_required', error: null },
          }),
        ],
      }),
    );
    renderPage();
    expect(await screen.findByText('UV needs you to sign in again')).toBeInTheDocument();
  });

  it('shows a sign-in failure inside the banner and does not sync', async () => {
    getTracker.mockResolvedValue(
      tracker({ universities: [university({ loginState: 'expired' })] }),
    );
    lmsSignIn.mockRejectedValue({
      kind: 'auth',
      message: 'Login window closed',
    });
    renderPage();
    await userEvent.click(await screen.findByRole('button', { name: 'Sign in' }));
    expect(await screen.findByText('Login window closed')).toBeInTheDocument();
    expect(startSync).not.toHaveBeenCalled();
  });

  it('summarises a finished sync and dismisses all changes', async () => {
    setSync({
      lastResults: [
        {
          universityId: 1,
          universityName: 'UV',
          outcome: 'success',
          newItems: 2,
          dueChanged: 0,
          removed: 0,
          error: null,
        },
        {
          universityId: 2,
          universityName: 'UAH',
          outcome: 'failed',
          newItems: 0,
          dueChanged: 0,
          removed: 0,
          error: 'Timeout',
        },
      ],
    });
    dismissAllChanges.mockResolvedValue(0);
    renderPage();
    expect(
      await screen.findByText('Sync finished: 2 new, 0 date changed, 0 removed'),
    ).toBeInTheDocument();
    expect(screen.getByText('UAH: Timeout')).toBeInTheDocument();
    await userEvent.click(screen.getByRole('button', { name: 'Dismiss all changes' }));
    await waitFor(() => expect(dismissAllChanges).toHaveBeenCalledWith(null));
    await userEvent.click(screen.getByRole('button', { name: 'Close' }));
    expect(clearResults).toHaveBeenCalled();
  });

  it('reports an interrupted sync', async () => {
    setSync({ lastResults: [] });
    renderPage();
    expect(await screen.findByText('The sync was interrupted. Try again.')).toBeInTheDocument();
  });

  it('opens the details modal for an item', async () => {
    getTracker.mockResolvedValue(
      tracker({
        items: [item({ id: 2, title: 'Final project', dueAt: daysFromNow(30) })],
      }),
    );
    renderPage();
    await userEvent.click(await screen.findByRole('button', { name: 'Final project' }));
    expect(await screen.findByRole('dialog')).toBeInTheDocument();
  });

  it('closes the details modal when the item leaves the list', async () => {
    getTracker.mockResolvedValue(
      tracker({
        items: [item({ id: 2, title: 'Final project', dueAt: daysFromNow(30) })],
      }),
    );
    renderPage();
    await userEvent.click(await screen.findByRole('button', { name: 'Final project' }));
    expect(await screen.findByRole('dialog')).toBeInTheDocument();
    getTracker.mockResolvedValue(tracker({ items: [] }));
    await userEvent.click(screen.getByRole('checkbox', { name: 'Show removed' }));
    await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument());
  });

  it('does not reopen the details modal when a hidden item comes back into the list', async () => {
    const shown = item({ id: 2, title: 'Final project', dueAt: daysFromNow(30) });
    getTracker.mockResolvedValue(tracker({ items: [shown] }));
    renderPage();
    await userEvent.click(await screen.findByRole('button', { name: 'Final project' }));
    expect(await screen.findByRole('dialog')).toBeInTheDocument();

    getTracker.mockResolvedValue(tracker({ items: [] }));
    await userEvent.click(screen.getByRole('button', { name: 'Hide' }));
    await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument());

    getTracker.mockResolvedValue(tracker({ items: [{ ...shown, hidden: true }] }));
    await userEvent.click(screen.getByRole('checkbox', { name: 'Show hidden' }));
    expect(await screen.findByRole('button', { name: 'Final project' })).toBeInTheDocument();
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
  });
});
