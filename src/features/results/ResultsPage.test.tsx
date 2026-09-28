import { beforeEach, describe, expect, it, vi } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import ResultsPage from './ResultsPage';
import type { AccountDto, Page, ResultDto, UniversityDto } from '../../lib/api/types';

const { getResults, getAccounts, getCatalog, setResultSeen } = vi.hoisted(() => ({
  getResults: vi.fn(),
  getAccounts: vi.fn(),
  getCatalog: vi.fn(),
  setResultSeen: vi.fn(),
}));

vi.mock('../../lib/api/client', () => ({
  api: { getResults, getAccounts, getCatalog, setResultSeen },
  toCommandError: (e: unknown) => ({ kind: 'internal', message: String(e) }),
}));

vi.mock('@tauri-apps/plugin-opener', () => ({ openUrl: vi.fn() }));

function resultFixture(overrides: Partial<ResultDto> = {}): ResultDto {
  return {
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

const accounts: AccountDto[] = [];
const universities: UniversityDto[] = [];

function renderPage() {
  const queryClient = new QueryClient();
  return render(
    <QueryClientProvider client={queryClient}>
      <ResultsPage />
    </QueryClientProvider>,
  );
}

beforeEach(() => {
  getResults.mockReset().mockResolvedValue(resultsPage([resultFixture()], 1));
  getAccounts.mockReset().mockResolvedValue(accounts);
  getCatalog.mockReset().mockResolvedValue(universities);
  setResultSeen.mockReset().mockResolvedValue(resultFixture({ seen: true }));
});

describe('ResultsPage', () => {
  it('applies category and seen filters from the modal and shows the active count', async () => {
    const user = userEvent.setup();
    renderPage();
    await screen.findByText('Exam date change');

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

    const subject = await screen.findByText('Exam date change');
    await user.click(subject);

    await user.click(screen.getByRole('button', { name: 'Mark as new' }));

    await waitFor(() => expect(setResultSeen).toHaveBeenCalledWith(1, false));
  });

  it('advances the offset when Next is clicked', async () => {
    getResults.mockResolvedValue(resultsPage([resultFixture()], 60));
    const user = userEvent.setup();
    renderPage();
    await screen.findByText('Exam date change');

    await user.click(screen.getByRole('button', { name: 'Next' }));

    await waitFor(() => expect(getResults).toHaveBeenCalledWith(expect.objectContaining({ offset: 25 })));
  });
});
