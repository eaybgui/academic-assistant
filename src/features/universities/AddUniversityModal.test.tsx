import { describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { MemoryRouter, Route, Routes } from 'react-router';
import AddUniversityModal from './AddUniversityModal';
import { api } from '../../lib/api/client';
import type { LmsCourseDto, LmsUniversityDto } from '../../lib/api/types';

vi.mock('../../lib/api/client', async (importOriginal) => {
  const actual = await importOriginal<typeof import('../../lib/api/client')>();
  return {
    ...actual,
    api: {
      addUniversity: vi.fn(),
      lmsSignIn: vi.fn(),
      lmsFetchCourses: vi.fn(),
      setTrackedCourses: vi.fn(),
    },
  };
});

const startSync = vi.fn();
vi.mock('../sync/useSync', () => ({
  useSync: () => ({ startSync, running: false, progress: null, lastResults: null, error: null, clearResults: vi.fn() }),
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

function course(overrides: Partial<LmsCourseDto>): LmsCourseDto {
  return {
    id: 1,
    universityId: 1,
    name: 'Álgebra Lineal',
    shortName: null,
    url: null,
    tracked: false,
    available: true,
    isNew: false,
    ...overrides,
  };
}

function renderModal(onClose = vi.fn()) {
  const queryClient = new QueryClient();
  render(
    <QueryClientProvider client={queryClient}>
      <MemoryRouter initialEntries={['/universities']}>
        <Routes>
          <Route path="/universities" element={<AddUniversityModal open onClose={onClose} />} />
          <Route path="/" element={<p>tracker</p>} />
        </Routes>
      </MemoryRouter>
    </QueryClientProvider>,
  );
  return onClose;
}

async function signInThroughWizard() {
  mockedApi.addUniversity.mockResolvedValue(university({ loginState: 'never' }));
  mockedApi.lmsSignIn.mockResolvedValue(university({ loginState: 'ok', accountLabel: 'Ana Pérez' }));
  fireEvent.change(screen.getByLabelText('Name'), { target: { value: 'UV' } });
  fireEvent.change(screen.getByLabelText('Platform URL'), { target: { value: 'aulavirtual.uv.es' } });
  fireEvent.click(screen.getByRole('button', { name: 'Continue' }));
  fireEvent.click(await screen.findByRole('button', { name: 'Sign in' }));
  await screen.findByText('Connected as Ana Pérez');
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

  describe('course selection', () => {
    const COURSES = [course({ id: 1, name: 'Álgebra Lineal' }), course({ id: 2, name: 'Cálculo' })];

    it('loads the courses with all selected, saves the selection, syncs and goes to the tracker', async () => {
      mockedApi.lmsFetchCourses.mockResolvedValue(COURSES);
      mockedApi.setTrackedCourses.mockResolvedValue(COURSES);
      startSync.mockResolvedValue(undefined);
      const onClose = renderModal();
      await signInThroughWizard();

      fireEvent.click(screen.getByRole('button', { name: 'Choose courses' }));
      expect(await screen.findByRole('checkbox', { name: /Álgebra Lineal/ })).toBeChecked();
      expect(screen.getByRole('checkbox', { name: /Cálculo/ })).toBeChecked();
      expect(mockedApi.lmsFetchCourses).toHaveBeenCalledWith(1);

      fireEvent.click(screen.getByRole('checkbox', { name: /Cálculo/ }));
      fireEvent.click(screen.getByRole('button', { name: 'Save and sync' }));

      expect(await screen.findByText('tracker')).toBeInTheDocument();
      expect(mockedApi.setTrackedCourses).toHaveBeenCalledWith(1, [1]);
      expect(startSync).toHaveBeenCalledWith(1);
      expect(onClose).toHaveBeenCalled();
    });

    it('shows a save error inline and lets the user retry', async () => {
      mockedApi.lmsFetchCourses.mockResolvedValue(COURSES);
      mockedApi.setTrackedCourses
        .mockRejectedValueOnce({ kind: 'storage', message: 'Could not save the courses' })
        .mockResolvedValue(COURSES);
      startSync.mockResolvedValue(undefined);
      renderModal();
      await signInThroughWizard();

      fireEvent.click(screen.getByRole('button', { name: 'Choose courses' }));
      await screen.findByRole('checkbox', { name: /Cálculo/ });
      fireEvent.click(screen.getByRole('button', { name: 'Save and sync' }));
      expect(await screen.findByText('Could not save the courses')).toBeInTheDocument();

      fireEvent.click(screen.getByRole('button', { name: 'Save and sync' }));
      await waitFor(() => expect(mockedApi.setTrackedCourses).toHaveBeenCalledTimes(2));
      expect(await screen.findByText('tracker')).toBeInTheDocument();
    });

    it('shows the load error inline and lets the user retry', async () => {
      mockedApi.lmsFetchCourses
        .mockRejectedValueOnce({ kind: 'network', message: 'Could not reach the platform' })
        .mockResolvedValue(COURSES);
      renderModal();
      await signInThroughWizard();

      fireEvent.click(screen.getByRole('button', { name: 'Choose courses' }));
      expect(await screen.findByText('Could not reach the platform')).toBeInTheDocument();

      fireEvent.click(screen.getByRole('button', { name: 'Choose courses' }));
      expect(await screen.findByRole('checkbox', { name: /Álgebra Lineal/ })).toBeChecked();
    });
  });
});
