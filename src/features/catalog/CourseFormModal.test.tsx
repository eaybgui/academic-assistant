import { describe, expect, it } from 'vitest';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { vi } from 'vitest';
import CourseFormModal from './CourseFormModal';
import { api } from '../../lib/api/client';

vi.mock('../../lib/api/client', async (importOriginal) => {
  const actual = await importOriginal<typeof import('../../lib/api/client')>();
  return {
    ...actual,
    api: {
      addCourse: vi.fn(),
      updateCourse: vi.fn(),
    },
  };
});

const mockedApi = vi.mocked(api, true);

function renderModal() {
  const queryClient = new QueryClient();
  return render(
    <QueryClientProvider client={queryClient}>
      <CourseFormModal open universityId={5} course={null} onClose={() => {}} />
    </QueryClientProvider>,
  );
}

describe('CourseFormModal', () => {
  it('parses comma-separated aliases when adding a course', async () => {
    mockedApi.addCourse.mockResolvedValue({ id: 1, universityId: 5, name: 'Algebra', aliases: ['ALG', 'LinAlg'] });

    renderModal();

    fireEvent.change(screen.getByLabelText('Name'), { target: { value: 'Algebra' } });
    fireEvent.change(screen.getByLabelText('Aliases'), { target: { value: ' ALG, LinAlg ,  ' } });
    fireEvent.click(screen.getByRole('button', { name: 'Save' }));

    await waitFor(() => expect(mockedApi.addCourse).toHaveBeenCalledWith(5, 'Algebra', ['ALG', 'LinAlg']));
  });
});
