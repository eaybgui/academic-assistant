import { beforeEach, describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import RunsPage from './RunsPage';
import type { RunDto } from '../../lib/api/types';

const { getRunHistory } = vi.hoisted(() => ({ getRunHistory: vi.fn() }));

vi.mock('../../lib/api/client', () => ({
  api: { getRunHistory },
  toCommandError: (e: unknown) => ({ kind: 'internal', message: String(e) }),
}));

function runFixture(): RunDto {
  return {
    id: 9,
    trigger: 'manual',
    startedAt: '2026-09-28T09:14:00Z',
    finishedAt: '2026-09-28T09:16:15Z',
    status: 'partial',
    accountsProcessed: 2,
    counters: { found: 10, filtered: 2, analyzed: 8, failed: 1, resultsCreated: 5 },
    errorSummary: '1 of 2 accounts failed',
    accounts: [
      {
        accountId: 1,
        accountName: 'UV IMAP',
        status: 'failed',
        counters: { found: 5, filtered: 1, analyzed: 3, failed: 1, resultsCreated: 2 },
        errorKind: 'connection',
        errorMessage: 'Could not reach the IMAP server',
      },
      {
        accountId: 2,
        accountName: 'UAH Graph',
        status: 'success',
        counters: { found: 5, filtered: 1, analyzed: 5, failed: 0, resultsCreated: 3 },
        errorKind: null,
        errorMessage: null,
      },
    ],
  };
}

function renderPage() {
  const queryClient = new QueryClient();
  return render(
    <QueryClientProvider client={queryClient}>
      <RunsPage />
    </QueryClientProvider>,
  );
}

beforeEach(() => {
  getRunHistory.mockReset().mockResolvedValue([runFixture()]);
});

describe('RunsPage', () => {
  it('is titled like its navigation entry', async () => {
    renderPage();
    expect(await screen.findByRole('heading', { level: 1, name: 'Email runs' })).toBeInTheDocument();
  });

  it('renders the run history with status and counters', async () => {
    renderPage();

    expect(await screen.findByText('Partial')).toBeInTheDocument();
    expect(getRunHistory).toHaveBeenCalledWith(50);
  });

  it('expands a run and shows the failed account with its error message', async () => {
    const user = userEvent.setup();
    renderPage();

    await screen.findByText('Partial');
    await user.click(screen.getByRole('button', { name: /Partial/ }));

    // The error kind and message render as adjacent text within the same
    // element ("Connection error: Could not reach the IMAP server"), so a
    // regex substring match is used instead of an exact string match.
    expect(await screen.findByText(/Could not reach the IMAP server/)).toBeInTheDocument();
    expect(screen.getByText(/UV IMAP/)).toBeInTheDocument();
    expect(screen.getByText(/Connection error/)).toBeInTheDocument();
  });

  it('shows an empty state when there is no history', async () => {
    getRunHistory.mockResolvedValue([]);
    renderPage();

    expect(await screen.findByText('No checks have run yet.')).toBeInTheDocument();
  });
});
