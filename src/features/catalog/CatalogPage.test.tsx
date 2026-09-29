import { describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import CatalogPage from './CatalogPage';
import { api } from '../../lib/api/client';
import type { UniversityDto } from '../../lib/api/types';

vi.mock('../../lib/api/client', async (importOriginal) => {
  const actual = await importOriginal<typeof import('../../lib/api/client')>();
  return {
    ...actual,
    api: {
      getCatalog: vi.fn(),
      addUniversity: vi.fn(),
      updateUniversity: vi.fn(),
      removeUniversity: vi.fn(),
      addCourse: vi.fn(),
      updateCourse: vi.fn(),
      removeCourse: vi.fn(),
    },
  };
});

const mockedApi = vi.mocked(api, true);

const catalog: UniversityDto[] = [
  {
    id: 4,
    name: 'UV',
    courses: [{ id: 9, universityId: 4, name: 'Algebra', aliases: ['Linear Algebra I'] }],
  },
];

function renderPage() {
  const queryClient = new QueryClient();
  return render(
    <QueryClientProvider client={queryClient}>
      <CatalogPage />
    </QueryClientProvider>,
  );
}

describe('CatalogPage', () => {
  it('discards unsaved edits when the university modal is cancelled and reopened', async () => {
    mockedApi.getCatalog.mockResolvedValue(catalog);

    renderPage();

    fireEvent.click(await screen.findByRole('button', { name: 'Edit' }));
    fireEvent.change(screen.getByLabelText('Name'), { target: { value: 'Unsaved name' } });
    expect(screen.getByLabelText('Name')).toHaveValue('Unsaved name');

    fireEvent.click(screen.getByRole('button', { name: 'Cancel' }));
    fireEvent.click(screen.getByRole('button', { name: 'Edit' }));

    expect(screen.getByLabelText('Name')).toHaveValue('UV');
  });

  it('discards unsaved edits when the course modal is cancelled and reopened', async () => {
    mockedApi.getCatalog.mockResolvedValue(catalog);

    renderPage();

    fireEvent.click(await screen.findByRole('button', { name: 'Edit Algebra' }));
    fireEvent.change(screen.getByLabelText('Name'), { target: { value: 'Unsaved name' } });
    expect(screen.getByLabelText('Name')).toHaveValue('Unsaved name');

    fireEvent.click(screen.getByRole('button', { name: 'Cancel' }));
    fireEvent.click(screen.getByRole('button', { name: 'Edit Algebra' }));

    expect(screen.getByLabelText('Name')).toHaveValue('Algebra');
  });
});
