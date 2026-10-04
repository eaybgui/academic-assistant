import { describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import UniversityFormModal from './UniversityFormModal';
import { api } from '../../lib/api/client';
import type { UniversityDto } from '../../lib/api/types';

vi.mock('../../lib/api/client', async (importOriginal) => {
  const actual = await importOriginal<typeof import('../../lib/api/client')>();
  return {
    ...actual,
    api: {
      addUniversity: vi.fn(),
      updateUniversity: vi.fn(),
    },
  };
});

const mockedApi = vi.mocked(api, true);

const existingUniversity: UniversityDto = { id: 4, name: 'UV', courses: [] };

function renderModal(university: UniversityDto | null = null) {
  const queryClient = new QueryClient();
  return render(
    <QueryClientProvider client={queryClient}>
      <UniversityFormModal open university={university} onClose={() => {}} />
    </QueryClientProvider>,
  );
}

describe('UniversityFormModal', () => {
  it('requires a name when adding a university', () => {
    renderModal();

    fireEvent.click(screen.getByRole('button', { name: 'Save' }));

    expect(screen.getByText('University name is required.')).toBeInTheDocument();
    expect(mockedApi.addUniversity).not.toHaveBeenCalled();
  });

  it('prefills the form with the university being edited and sends the edited payload via updateUniversity', async () => {
    mockedApi.updateUniversity.mockResolvedValue({ ...existingUniversity, name: 'Universitat de Valencia' });

    renderModal(existingUniversity);

    expect(screen.getByLabelText('Name')).toHaveValue('UV');

    fireEvent.change(screen.getByLabelText('Name'), { target: { value: 'Universitat de Valencia' } });
    fireEvent.click(screen.getByRole('button', { name: 'Save' }));

    await waitFor(() => expect(mockedApi.updateUniversity).toHaveBeenCalledWith(4, 'Universitat de Valencia'));
  });
});
