import { describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { MemoryRouter } from 'react-router';
import UniversitiesPage from './UniversitiesPage';
import { api } from '../../lib/api/client';
import type { LmsUniversityDto } from '../../lib/api/types';

vi.mock('../../lib/api/client', async (importOriginal) => {
  const actual = await importOriginal<typeof import('../../lib/api/client')>();
  return {
    ...actual,
    api: {
      getUniversities: vi.fn(),
      addUniversity: vi.fn(),
      renameUniversity: vi.fn(),
      removeUniversity: vi.fn(),
      lmsSignIn: vi.fn(),
      lmsSignOut: vi.fn(),
      lmsCheckSession: vi.fn(),
    },
  };
});

vi.mock('../sync/useSync', () => ({
  useSync: () => ({
    startSync: vi.fn(),
    running: false,
    progress: null,
    lastResults: null,
    error: null,
    clearResults: vi.fn(),
  }),
}));

const mockedApi = vi.mocked(api, true);

function university(overrides: Partial<LmsUniversityDto> = {}): LmsUniversityDto {
  return {
    id: 1,
    name: 'UV',
    platform: 'moodle',
    baseUrl: 'https://aulavirtual.uv.es',
    loginState: 'never',
    accountLabel: null,
    lastSync: { at: null, outcome: null, error: null },
    courses: { tracked: 0, total: 0, unreviewed: 0 },
    ...overrides,
  };
}

function renderPage(state?: { openAdd: true }) {
  const queryClient = new QueryClient();
  return render(
    <QueryClientProvider client={queryClient}>
      <MemoryRouter initialEntries={[{ pathname: '/universities', state }]}>
        <UniversitiesPage />
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

describe('UniversitiesPage', () => {
  it('lists universities with their connection state', async () => {
    mockedApi.getUniversities.mockResolvedValue([
      university({ id: 1, name: 'UV', loginState: 'ok', accountLabel: 'Ana Pérez' }),
      university({
        id: 2,
        name: 'UAH',
        platform: 'blackboard',
        baseUrl: 'https://uah.blackboard.com',
        loginState: 'expired',
      }),
    ]);
    renderPage();
    expect(await screen.findByText('Connected as Ana Pérez')).toBeInTheDocument();
    expect(screen.getByText('Session expired')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Sign in to UAH' })).toBeInTheDocument();
  });

  it('shows an empty state with the add action', async () => {
    mockedApi.getUniversities.mockResolvedValue([]);
    renderPage();
    expect(await screen.findByText('No universities yet')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Add university' })).toBeInTheDocument();
  });

  it('checks the connection and shows the error inline', async () => {
    mockedApi.getUniversities.mockResolvedValue([university({ id: 1, name: 'UV', loginState: 'ok' })]);
    mockedApi.lmsCheckSession.mockRejectedValue({ kind: 'auth', message: 'UV needs you to sign in again' });
    renderPage();
    fireEvent.click(await screen.findByRole('button', { name: 'Check connection to UV' }));
    expect(await screen.findByText('UV needs you to sign in again')).toBeInTheDocument();
  });

  it('confirms before deleting', async () => {
    mockedApi.getUniversities.mockResolvedValue([university({ id: 1, name: 'UV' })]);
    mockedApi.removeUniversity.mockResolvedValue(undefined);
    renderPage();
    fireEvent.click(await screen.findByRole('button', { name: 'Delete UV' }));
    expect(screen.getByText('Deletes this university, its courses and your saved sign-in.')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Delete' }));
    await waitFor(() => expect(mockedApi.removeUniversity).toHaveBeenCalledWith(1));
  });

  it('opens the add wizard when navigated to with openAdd state', async () => {
    mockedApi.getUniversities.mockResolvedValue([]);
    renderPage({ openAdd: true });
    expect(await screen.findByRole('dialog', { name: 'Add university' })).toBeInTheDocument();
  });

  it('does not open the add wizard by default', async () => {
    mockedApi.getUniversities.mockResolvedValue([]);
    renderPage();
    await screen.findByText('No universities yet');
    expect(screen.queryByRole('dialog', { name: 'Add university' })).not.toBeInTheDocument();
  });
});
