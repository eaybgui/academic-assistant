import { describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import AddUniversityModal from './AddUniversityModal';
import { api } from '../../lib/api/client';
import type { LmsUniversityDto } from '../../lib/api/types';

vi.mock('../../lib/api/client', async (importOriginal) => {
  const actual = await importOriginal<typeof import('../../lib/api/client')>();
  return {
    ...actual,
    api: {
      addUniversity: vi.fn(),
      lmsSignIn: vi.fn(),
    },
  };
});

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

function renderModal() {
  const queryClient = new QueryClient();
  return render(
    <QueryClientProvider client={queryClient}>
      <AddUniversityModal open onClose={vi.fn()} />
    </QueryClientProvider>,
  );
}

describe('AddUniversityModal', () => {
  it('saves, then signs in and shows the connected account', async () => {
    mockedApi.addUniversity.mockResolvedValue(university({ loginState: 'never' }));
    mockedApi.lmsSignIn.mockResolvedValue(university({ loginState: 'ok', accountLabel: 'Ana Pérez' }));

    renderModal();
    fireEvent.change(screen.getByLabelText('Name'), { target: { value: 'UV' } });
    fireEvent.change(screen.getByLabelText('Platform'), { target: { value: 'moodle' } });
    fireEvent.change(screen.getByLabelText('Platform URL'), { target: { value: 'aulavirtual.uv.es/my' } });
    fireEvent.click(screen.getByRole('button', { name: 'Continue' }));

    expect(await screen.findByText('https://aulavirtual.uv.es')).toBeInTheDocument();
    expect(mockedApi.addUniversity).toHaveBeenCalledWith({
      name: 'UV',
      platform: 'moodle',
      baseUrl: 'aulavirtual.uv.es/my',
    });

    fireEvent.click(screen.getByRole('button', { name: 'Sign in' }));
    expect(await screen.findByText('Connected as Ana Pérez')).toBeInTheDocument();
  });

  it('shows the probe error inline and does not save', async () => {
    mockedApi.addUniversity.mockRejectedValue({ kind: 'validation', message: 'This URL is not a Moodle site' });
    renderModal();
    fireEvent.change(screen.getByLabelText('Name'), { target: { value: 'UV' } });
    fireEvent.change(screen.getByLabelText('Platform URL'), { target: { value: 'example.com' } });
    fireEvent.click(screen.getByRole('button', { name: 'Continue' }));
    expect(await screen.findByText('This URL is not a Moodle site')).toBeInTheDocument();
  });

  it('keeps the dialog usable after a cancelled sign-in', async () => {
    mockedApi.addUniversity.mockResolvedValue(university({ loginState: 'never' }));
    mockedApi.lmsSignIn.mockRejectedValueOnce({ kind: 'auth', message: 'Sign-in cancelled' });
    renderModal();
    fireEvent.change(screen.getByLabelText('Name'), { target: { value: 'UV' } });
    fireEvent.change(screen.getByLabelText('Platform URL'), { target: { value: 'aulavirtual.uv.es' } });
    fireEvent.click(screen.getByRole('button', { name: 'Continue' }));
    fireEvent.click(await screen.findByRole('button', { name: 'Sign in' }));
    expect(await screen.findByText('Sign-in cancelled')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Sign in' })).toBeEnabled();
  });
});
