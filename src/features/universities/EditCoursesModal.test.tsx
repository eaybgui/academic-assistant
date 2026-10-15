import { useState } from 'react';
import { describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import EditCoursesModal from './EditCoursesModal';
import { api } from '../../lib/api/client';
import { queryKeys } from '../../lib/api/queryKeys';
import type { LmsCourseDto, LmsUniversityDto } from '../../lib/api/types';

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
vi.mock('../sync/useSync', () => ({
  useSync: () => ({ startSync, running: false, progress: null, lastResults: null, error: null, clearResults: vi.fn() }),
}));

const mockedApi = vi.mocked(api, true);

const UNIVERSITY: LmsUniversityDto = {
  id: 1,
  name: 'UV',
  platform: 'moodle',
  baseUrl: 'https://campus.demo.example',
  loginState: 'ok',
  accountLabel: 'Ana Pérez',
  lastSync: { at: null, outcome: null, error: null },
  courses: { tracked: 1, total: 2, unreviewed: 0 },
};

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

const COURSES = [course({ id: 1, name: 'Álgebra Lineal', tracked: true }), course({ id: 2, name: 'Cálculo' })];

function renderModal(onClose = vi.fn(), queryClient = new QueryClient()) {
  render(
    <QueryClientProvider client={queryClient}>
      <EditCoursesModal open university={UNIVERSITY} onClose={onClose} />
    </QueryClientProvider>,
  );
  return onClose;
}

/** Like the real parent: `open` follows `onClose`. */
function ClosableHost() {
  const [open, setOpen] = useState(true);
  return <EditCoursesModal open={open} university={UNIVERSITY} onClose={() => setOpen(false)} />;
}

describe('EditCoursesModal', () => {
  it('falls back to the stored courses and shows the error when the platform fetch fails', async () => {
    mockedApi.lmsFetchCourses.mockRejectedValue({ kind: 'auth', message: 'UV needs you to sign in again' });
    mockedApi.getCourses.mockResolvedValue(COURSES);
    renderModal();

    expect(await screen.findByText('UV needs you to sign in again')).toBeInTheDocument();
    expect(await screen.findByRole('checkbox', { name: /Álgebra Lineal/ })).toBeChecked();
    expect(screen.getByRole('checkbox', { name: /Cálculo/ })).not.toBeChecked();
    expect(mockedApi.getCourses).toHaveBeenCalledWith(1);
  });

  it('pre-selects the tracked courses, saves, starts a sync and closes', async () => {
    mockedApi.lmsFetchCourses.mockResolvedValue(COURSES);
    mockedApi.setTrackedCourses.mockResolvedValue(COURSES);
    startSync.mockResolvedValue(undefined);
    const onClose = renderModal();

    fireEvent.click(await screen.findByRole('checkbox', { name: /Cálculo/ }));
    fireEvent.click(screen.getByRole('button', { name: 'Save' }));

    await waitFor(() => expect(onClose).toHaveBeenCalled());
    expect(mockedApi.setTrackedCourses).toHaveBeenCalledWith(1, [1, 2]);
    expect(startSync).toHaveBeenCalledWith(1);
    expect(mockedApi.getCourses).not.toHaveBeenCalled();
  });

  it('keeps the modal open and shows the error when saving fails', async () => {
    mockedApi.lmsFetchCourses.mockResolvedValue(COURSES);
    mockedApi.setTrackedCourses.mockRejectedValue({ kind: 'storage', message: 'Could not save the courses' });
    startSync.mockClear();
    const onClose = renderModal();

    await screen.findByRole('checkbox', { name: /Cálculo/ });
    fireEvent.click(screen.getByRole('button', { name: 'Save' }));

    expect(await screen.findByText('Could not save the courses')).toBeInTheDocument();
    expect(startSync).not.toHaveBeenCalled();
    expect(onClose).not.toHaveBeenCalled();
  });

  it('fetches the course list from the platform once per open, also after saving', async () => {
    mockedApi.lmsFetchCourses.mockClear();
    mockedApi.lmsFetchCourses.mockResolvedValue(COURSES);
    mockedApi.setTrackedCourses.mockResolvedValue(COURSES);
    startSync.mockResolvedValue(undefined);
    render(
      <QueryClientProvider client={new QueryClient()}>
        <ClosableHost />
      </QueryClientProvider>,
    );

    fireEvent.click(await screen.findByRole('checkbox', { name: /Cálculo/ }));
    fireEvent.click(screen.getByRole('button', { name: 'Save' }));

    await waitFor(() => expect(startSync).toHaveBeenCalledWith(1));
    await waitFor(() => expect(screen.queryByRole('button', { name: 'Save' })).not.toBeInTheDocument());
    expect(mockedApi.lmsFetchCourses).toHaveBeenCalledTimes(1);
  });

  it('keeps the course list under the shared courses query key', async () => {
    mockedApi.lmsFetchCourses.mockResolvedValue(COURSES);
    const queryClient = new QueryClient();
    renderModal(vi.fn(), queryClient);

    await screen.findByRole('checkbox', { name: /Cálculo/ });
    expect(queryClient.getQueryData(queryKeys.courses(1))).toBeDefined();
  });

  it('refreshes the universities when the platform fetch fails, so the card shows the new login state', async () => {
    mockedApi.lmsFetchCourses.mockRejectedValue({ kind: 'auth', message: 'UV needs you to sign in again' });
    mockedApi.getCourses.mockResolvedValue(COURSES);
    const queryClient = new QueryClient();
    const invalidate = vi.spyOn(queryClient, 'invalidateQueries');
    renderModal(vi.fn(), queryClient);

    await screen.findByText('UV needs you to sign in again');
    expect(invalidate).toHaveBeenCalledWith({ queryKey: queryKeys.universities });
  });

  it('does not refresh the universities when the platform fetch succeeds', async () => {
    mockedApi.lmsFetchCourses.mockResolvedValue(COURSES);
    const queryClient = new QueryClient();
    const invalidate = vi.spyOn(queryClient, 'invalidateQueries');
    renderModal(vi.fn(), queryClient);

    await screen.findByRole('checkbox', { name: /Cálculo/ });
    expect(invalidate).not.toHaveBeenCalled();
  });
});
