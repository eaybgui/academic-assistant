import { beforeEach, describe, expect, it, vi } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import AnnouncementsPage from './AnnouncementsPage';
import { CheckRunProvider } from '../check-run/CheckRunProvider';
import type { AccountDto, DashboardDto, Page, ResultDto, RunProgressEvent, UniversityDto } from '../../lib/api/types';

const { getDashboard, getResults, getAccounts, getCatalog, checkNow, setResultSeen, markAllSeen } = vi.hoisted(() => ({
  getDashboard: vi.fn(),
  getResults: vi.fn(),
  getAccounts: vi.fn(),
  getCatalog: vi.fn(),
  checkNow: vi.fn(),
  setResultSeen: vi.fn(),
  markAllSeen: vi.fn(),
}));

vi.mock('../../lib/api/client', () => ({
  api: { getDashboard, getResults, getAccounts, getCatalog, checkNow, setResultSeen, markAllSeen },
  toCommandError: (e: unknown) =>
    typeof e === 'object' && e !== null && 'kind' in e ? e : { kind: 'internal', message: String(e) },
}));

const { onRunProgress, onRunFinished } = vi.hoisted(() => ({
  onRunProgress: vi.fn(),
  onRunFinished: vi.fn(),
}));

vi.mock('../../lib/api/events', () => ({ onRunProgress, onRunFinished }));

vi.mock('@tauri-apps/plugin-opener', () => ({ openUrl: vi.fn() }));

function dashboardFixture(overrides: Partial<DashboardDto> = {}): DashboardDto {
  return {
    lastRun: null,
    runningRunId: null,
    attention: [],
    counts: { newTotal: 2, tasks: 1, exams: 0, events: 0, changesAndNotices: 1 },
    newResults: [],
    ...overrides,
  };
}

function resultFixture(overrides: Partial<ResultDto> = {}): ResultDto {
  return {
    id: 1,
    accountId: 1,
    accountName: 'Cuenta Demo',
    subject: 'Cambio de fecha del examen',
    senderName: 'Secretaría',
    senderEmail: 'secretaria@demo.example',
    universityName: 'Universidad Demo',
    courseName: 'Álgebra Lineal',
    category: 'exam',
    priority: 'high',
    actionRequired: false,
    possibleChange: false,
    dates: [],
    receivedAt: '2026-09-27T08:00:00Z',
    detectedAt: '2026-09-27T08:05:00Z',
    sourceUrl: null,
    messageIdHeader: null,
    seen: false,
    seenAt: null,
    ...overrides,
  };
}

function resultsPage(items: ResultDto[], total: number): Page<ResultDto> {
  return { items, total };
}

const SUBJECT = 'Cambio de fecha del examen';
const accounts: AccountDto[] = [];
const universities: UniversityDto[] = [];

function renderPage() {
  const queryClient = new QueryClient();
  return render(
    <QueryClientProvider client={queryClient}>
      <CheckRunProvider>
        <AnnouncementsPage />
      </CheckRunProvider>
    </QueryClientProvider>,
  );
}

beforeEach(() => {
  getDashboard.mockReset().mockResolvedValue(dashboardFixture());
  getResults.mockReset().mockResolvedValue(resultsPage([resultFixture()], 1));
  getAccounts.mockReset().mockResolvedValue(accounts);
  getCatalog.mockReset().mockResolvedValue(universities);
  checkNow.mockReset();
  setResultSeen.mockReset().mockResolvedValue(resultFixture({ seen: true }));
  markAllSeen.mockReset().mockResolvedValue(0);
  onRunProgress.mockReset().mockResolvedValue(vi.fn());
  onRunFinished.mockReset().mockResolvedValue(vi.fn());
});

describe('AnnouncementsPage header', () => {
  it('renders the heading, the five counters and the results list', async () => {
    renderPage();

    expect(await screen.findByText(SUBJECT)).toBeInTheDocument();
    expect(screen.getByRole('heading', { name: 'Announcements' })).toBeInTheDocument();
    expect(await screen.findByText('Changes and notices')).toBeInTheDocument();
    expect(screen.getByText('New')).toBeInTheDocument();
    expect(screen.getByText('Tasks')).toBeInTheDocument();
    expect(screen.getByText('Exams')).toBeInTheDocument();
    expect(screen.getByText('Events')).toBeInTheDocument();
    expect(getDashboard).toHaveBeenCalledWith('upcoming');
  });

  it('shows "No checks yet." when there is no last run', async () => {
    renderPage();

    expect(await screen.findByText('No checks yet.')).toBeInTheDocument();
  });

  it('calls checkNow and shows progress from a run:progress event', async () => {
    checkNow.mockResolvedValue(7);
    let progressCb: ((e: RunProgressEvent) => void) | undefined;
    onRunProgress.mockImplementation((cb: (e: RunProgressEvent) => void) => {
      progressCb = cb;
      return Promise.resolve(vi.fn());
    });

    const user = userEvent.setup();
    renderPage();
    await screen.findByText(SUBJECT);

    await user.click(screen.getByRole('button', { name: 'Check now' }));

    await waitFor(() => expect(checkNow).toHaveBeenCalled());
    await waitFor(() => expect(progressCb).toBeDefined());

    progressCb?.({ runId: 7, accountId: 1, accountName: 'UV IMAP', stage: 'analyzing', processed: 12, total: 40 });

    expect(await screen.findByText('UV IMAP: Analyzing 12/40')).toBeInTheDocument();
  });

  it('shows the "track a course first" validation error inline', async () => {
    checkNow.mockRejectedValue({ kind: 'validation', message: 'Track at least one course before checking email' });
    const user = userEvent.setup();
    renderPage();
    await screen.findByText(SUBJECT);

    await user.click(screen.getByRole('button', { name: 'Check now' }));

    expect(await screen.findByText(/Track at least one course before checking email/)).toBeInTheDocument();
  });

  it('calls markAllSeen when "Mark all as seen" is clicked', async () => {
    const user = userEvent.setup();
    renderPage();
    await screen.findByText(SUBJECT);

    await user.click(screen.getByRole('button', { name: 'Mark all as seen' }));

    await waitFor(() => expect(markAllSeen).toHaveBeenCalled());
  });

  it('renders attention banners', async () => {
    getDashboard.mockResolvedValue(
      dashboardFixture({
        attention: [{ accountId: 3, accountName: 'Cuenta Demo', message: 'Sign in to this account' }],
      }),
    );
    renderPage();

    expect(await screen.findByRole('alert')).toHaveTextContent('Cuenta Demo');
    expect(await screen.findByRole('alert')).toHaveTextContent('Sign in to this account');
  });
});

describe('AnnouncementsPage results list', () => {
  it('applies category and seen filters from the modal and shows the active count', async () => {
    const user = userEvent.setup();
    renderPage();
    await screen.findByText(SUBJECT);

    await user.click(screen.getByRole('button', { name: /Filters and sort/ }));
    await user.selectOptions(screen.getByLabelText('Category'), 'exam');
    await user.selectOptions(screen.getByLabelText('State'), 'false');
    await user.click(screen.getByRole('button', { name: 'Apply' }));

    await waitFor(() =>
      expect(getResults).toHaveBeenCalledWith(expect.objectContaining({ category: 'exam', seen: false })),
    );

    expect(screen.getByRole('button', { name: /Filters and sort/ })).toHaveTextContent('2');
  });

  it('calls setResultSeen(id, false) for "Mark as new" on an already-seen result', async () => {
    getResults.mockResolvedValue(resultsPage([resultFixture({ seen: true, seenAt: '2026-09-27T09:00:00Z' })], 1));
    const user = userEvent.setup();
    renderPage();

    await user.click(await screen.findByText(SUBJECT));
    await user.click(screen.getByRole('button', { name: 'Mark as new' }));

    await waitFor(() => expect(setResultSeen).toHaveBeenCalledWith(1, false));
  });

  it('advances the offset when Next is clicked', async () => {
    getResults.mockResolvedValue(resultsPage([resultFixture()], 60));
    const user = userEvent.setup();
    renderPage();
    await screen.findByText(SUBJECT);

    await user.click(screen.getByRole('button', { name: 'Next' }));

    await waitFor(() => expect(getResults).toHaveBeenCalledWith(expect.objectContaining({ offset: 25 })));
  });

  it('clamps back to the last page once total shrinks below the current page', async () => {
    getResults.mockResolvedValue(resultsPage([resultFixture()], 60));
    const user = userEvent.setup();
    renderPage();
    await screen.findByText(SUBJECT);

    await user.click(screen.getByRole('button', { name: 'Next' }));
    await waitFor(() => expect(getResults).toHaveBeenCalledWith(expect.objectContaining({ offset: 25 })));
    expect(screen.getByText('Page 2 of 3 (60 results)')).toBeInTheDocument();

    // The result set shrank (e.g. a filter applied elsewhere) to fewer than one page.
    getResults.mockResolvedValue(resultsPage([resultFixture()], 3));
    await user.click(screen.getByRole('button', { name: /Filters and sort/ }));
    await user.click(screen.getByRole('button', { name: 'Apply' }));

    await waitFor(() => expect(screen.getByText('Page 1 of 1 (3 results)')).toBeInTheDocument());
    await waitFor(() => expect(getResults).toHaveBeenCalledWith(expect.objectContaining({ offset: 0 })));
  });

  it('renders the subject as a keyboard-accessible expand toggle', async () => {
    renderPage();

    const toggle = await screen.findByRole('button', { name: new RegExp(SUBJECT) });
    expect(toggle).toHaveAttribute('aria-expanded', 'false');
  });
});
