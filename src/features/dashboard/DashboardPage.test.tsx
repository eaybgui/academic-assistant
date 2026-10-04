import { beforeEach, describe, expect, it, vi } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import DashboardPage from './DashboardPage';
import { CheckRunProvider } from '../check-run/CheckRunProvider';
import type { DashboardDto, RunProgressEvent } from '../../lib/api/types';

const { getDashboard, checkNow, setResultSeen, markAllSeen } = vi.hoisted(() => ({
  getDashboard: vi.fn(),
  checkNow: vi.fn(),
  setResultSeen: vi.fn(),
  markAllSeen: vi.fn(),
}));

vi.mock('../../lib/api/client', () => ({
  api: { getDashboard, checkNow, setResultSeen, markAllSeen },
  toCommandError: (e: unknown) => ({ kind: 'internal', message: String(e) }),
}));

const { onRunProgress, onRunFinished } = vi.hoisted(() => ({
  onRunProgress: vi.fn(),
  onRunFinished: vi.fn(),
}));

vi.mock('../../lib/api/events', () => ({ onRunProgress, onRunFinished }));

function dashboardFixture(overrides: Partial<DashboardDto> = {}): DashboardDto {
  return {
    lastRun: null,
    runningRunId: null,
    attention: [],
    counts: { newTotal: 2, tasks: 1, exams: 0, events: 0, changesAndNotices: 1 },
    newResults: [
      {
        id: 1,
        accountId: 1,
        accountName: 'UV',
        subject: 'Exam date change',
        senderName: 'Office',
        senderEmail: 'office@uv.es',
        universityName: 'UV',
        courseName: 'Algebra',
        category: 'exam',
        priority: 'high',
        actionRequired: true,
        possibleChange: true,
        dates: [{ date: '2026-10-27', time: '10:00', kind: 'exam' }],
        receivedAt: '2026-09-27T08:00:00Z',
        detectedAt: '2026-09-27T08:05:00Z',
        sourceUrl: null,
        messageIdHeader: '<abc@uv.es>',
        seen: false,
        seenAt: null,
      },
    ],
    ...overrides,
  };
}

function renderPage() {
  const queryClient = new QueryClient();
  return render(
    <QueryClientProvider client={queryClient}>
      <CheckRunProvider>
        <DashboardPage />
      </CheckRunProvider>
    </QueryClientProvider>,
  );
}

beforeEach(() => {
  getDashboard.mockReset().mockResolvedValue(dashboardFixture());
  checkNow.mockReset();
  setResultSeen.mockReset().mockResolvedValue(undefined);
  markAllSeen.mockReset().mockResolvedValue(0);
  onRunProgress.mockReset().mockResolvedValue(vi.fn());
  onRunFinished.mockReset().mockResolvedValue(vi.fn());
});

describe('DashboardPage', () => {
  it('renders the five counters and a result card', async () => {
    renderPage();

    expect(await screen.findByText('Exam date change')).toBeInTheDocument();
    expect(screen.getByText('New')).toBeInTheDocument();
    expect(screen.getByText('Tasks')).toBeInTheDocument();
    expect(screen.getByText('Exams')).toBeInTheDocument();
    expect(screen.getByText('Events')).toBeInTheDocument();
    expect(screen.getByText('Changes and notices')).toBeInTheDocument();
  });

  it('refetches with the new sort when the selector changes', async () => {
    const user = userEvent.setup();
    renderPage();
    await screen.findByText('Exam date change');

    expect(getDashboard).toHaveBeenCalledWith('upcoming');

    await user.selectOptions(screen.getByLabelText('Sort'), 'priority');

    await waitFor(() => expect(getDashboard).toHaveBeenCalledWith('priority'));
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
    await screen.findByText('Exam date change');

    await user.click(screen.getByRole('button', { name: 'Check now' }));

    await waitFor(() => expect(checkNow).toHaveBeenCalled());
    await waitFor(() => expect(progressCb).toBeDefined());

    progressCb?.({ runId: 7, accountId: 1, accountName: 'UV IMAP', stage: 'analyzing', processed: 12, total: 40 });

    expect(await screen.findByText('UV IMAP: Analyzing 12/40')).toBeInTheDocument();
  });

  it('calls setResultSeen(id, true) when Seen is clicked', async () => {
    const user = userEvent.setup();
    renderPage();
    await screen.findByText('Exam date change');

    await user.click(screen.getByRole('button', { name: 'Seen' }));

    await waitFor(() => expect(setResultSeen).toHaveBeenCalledWith(1, true));
  });

  it('calls markAllSeen when "Mark all as seen" is clicked', async () => {
    const user = userEvent.setup();
    renderPage();
    await screen.findByText('Exam date change');

    await user.click(screen.getByRole('button', { name: 'Mark all as seen' }));

    await waitFor(() => expect(markAllSeen).toHaveBeenCalled());
  });

  it('renders attention banners', async () => {
    getDashboard.mockResolvedValue(
      dashboardFixture({
        attention: [{ accountId: 3, accountName: 'UAH', message: 'Sign in to this account' }],
      }),
    );
    renderPage();

    expect(await screen.findByRole('alert')).toHaveTextContent('UAH');
    expect(await screen.findByRole('alert')).toHaveTextContent('Sign in to this account');
  });
});
