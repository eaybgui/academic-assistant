import { describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import UniversityCard from './UniversityCard';
import { api } from '../../lib/api/client';
import type { LmsUniversityDto } from '../../lib/api/types';

vi.mock('../../lib/api/client', async (importOriginal) => {
  const actual = await importOriginal<typeof import('../../lib/api/client')>();
  return {
    ...actual,
    api: {
      lmsFetchCourses: vi.fn(),
      getCourses: vi.fn(),
      setTrackedCourses: vi.fn(),
    },
  };
});

const startSync = vi.fn();
let running = false;
vi.mock('../sync/useSync', () => ({
  useSync: () => ({ startSync, running, progress: null, lastResults: null, error: null, clearResults: vi.fn() }),
}));

const mockedApi = vi.mocked(api, true);

function university(overrides: Partial<LmsUniversityDto> = {}): LmsUniversityDto {
  return {
    id: 1,
    name: 'UV',
    platform: 'moodle',
    baseUrl: 'https://aulavirtual.uv.es',
    loginState: 'ok',
    accountLabel: 'Ana Pérez',
    lastSync: { at: null, outcome: null, error: null },
    courses: { tracked: 3, total: 5, unreviewed: 0 },
    ...overrides,
  };
}

function renderCard(overrides: Partial<LmsUniversityDto> = {}) {
  render(
    <QueryClientProvider client={new QueryClient()}>
      <ul>
        <UniversityCard university={university(overrides)} />
      </ul>
    </QueryClientProvider>,
  );
}

describe('UniversityCard', () => {
  it('starts a sync of the university', () => {
    running = false;
    startSync.mockResolvedValue(undefined);
    renderCard();
    fireEvent.click(screen.getByRole('button', { name: 'Sync UV' }));
    expect(startSync).toHaveBeenCalledWith(1);
  });

  it('disables the sync button while a sync is running', () => {
    running = true;
    renderCard();
    expect(screen.getByRole('button', { name: 'Sync UV' })).toBeDisabled();
    running = false;
  });

  it('hides sync and edit actions when not signed in', () => {
    renderCard({ loginState: 'expired' });
    expect(screen.queryByRole('button', { name: 'Sync UV' })).not.toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Edit courses of UV' })).not.toBeInTheDocument();
  });

  it('shows the new courses link only when some courses were already reviewed', () => {
    renderCard({ courses: { total: 5, tracked: 3, unreviewed: 2 } });
    expect(screen.getByRole('button', { name: '2 new courses found' })).toBeInTheDocument();
  });

  it('does not show the new courses link when nothing was reviewed yet', () => {
    renderCard({ courses: { total: 5, tracked: 0, unreviewed: 5 } });
    expect(screen.queryByText(/new courses found/)).not.toBeInTheDocument();
  });

  it('opens the course editor from the card actions', async () => {
    mockedApi.lmsFetchCourses.mockResolvedValue([]);
    renderCard();
    fireEvent.click(screen.getByRole('button', { name: 'Edit courses of UV' }));
    expect(await screen.findByRole('heading', { name: 'Edit courses of UV' })).toBeInTheDocument();
    expect(mockedApi.lmsFetchCourses).toHaveBeenCalledWith(1);
  });

  it('shows the last sync time, and the error of a failed sync', () => {
    renderCard({
      lastSync: { at: new Date().toISOString(), outcome: 'failed', error: 'Session expired during the sync' },
    });
    expect(screen.getByText(/^Last synced today/)).toBeInTheDocument();
    expect(screen.getByText('Session expired during the sync')).toBeInTheDocument();
  });

  it('shows no last sync line before the first sync', () => {
    renderCard();
    expect(screen.queryByText(/Last synced/)).not.toBeInTheDocument();
  });
});
