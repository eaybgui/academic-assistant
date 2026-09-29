import { beforeEach, describe, expect, it, vi } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { CheckRunProvider } from './CheckRunProvider';
import { useCheckRun } from './useCheckRun';
import type { RunFinishedEvent } from '../../lib/api/types';

const { checkNow } = vi.hoisted(() => ({ checkNow: vi.fn() }));

vi.mock('../../lib/api/client', async (importOriginal) => {
  const actual = await importOriginal<typeof import('../../lib/api/client')>();
  return { ...actual, api: { ...actual.api, checkNow } };
});

const { onRunProgress, onRunFinished } = vi.hoisted(() => ({
  onRunProgress: vi.fn(),
  onRunFinished: vi.fn(),
}));

vi.mock('../../lib/api/events', () => ({ onRunProgress, onRunFinished }));

/** A minimal consumer standing in for "any route" - it doesn't render the dashboard at all. */
function TestConsumer() {
  const { running, progress, triggerCheck } = useCheckRun();
  return (
    <div>
      <p>{running ? `running:${progress ?? ''}` : 'idle'}</p>
      <button type="button" onClick={() => void triggerCheck()}>
        Trigger
      </button>
    </div>
  );
}

function renderWithProvider(queryClient: QueryClient) {
  return render(
    <QueryClientProvider client={queryClient}>
      <CheckRunProvider>
        <TestConsumer />
      </CheckRunProvider>
    </QueryClientProvider>,
  );
}

beforeEach(() => {
  checkNow.mockReset();
  onRunProgress.mockReset().mockResolvedValue(vi.fn());
  onRunFinished.mockReset().mockResolvedValue(vi.fn());
});

describe('CheckRunProvider', () => {
  it('does not leave the UI running when run:finished arrives before checkNow() resolves', async () => {
    let finishedCb: ((e: RunFinishedEvent) => void) | undefined;
    onRunFinished.mockImplementation((cb: (e: RunFinishedEvent) => void) => {
      finishedCb = cb;
      return Promise.resolve(vi.fn());
    });

    let resolveCheckNow: (id: number) => void = () => {};
    checkNow.mockImplementation(
      () =>
        new Promise<number>((resolve) => {
          resolveCheckNow = resolve;
        }),
    );

    const queryClient = new QueryClient();
    const user = userEvent.setup();
    renderWithProvider(queryClient);

    await waitFor(() => expect(finishedCb).toBeDefined());

    await user.click(screen.getByRole('button', { name: 'Trigger' }));
    await waitFor(() => expect(checkNow).toHaveBeenCalled());

    // The backend reports the run finished before our `checkNow()` call itself resolves with the
    // run id - the listener (subscribed since mount) sees it first.
    finishedCb?.({ runId: 42, status: 'success' });
    resolveCheckNow(42);

    await waitFor(() => expect(screen.getByText('idle')).toBeInTheDocument());
  });

  it('invalidates dashboard/results/runs queries on run:finished regardless of what is mounted', async () => {
    let finishedCb: ((e: RunFinishedEvent) => void) | undefined;
    onRunFinished.mockImplementation((cb: (e: RunFinishedEvent) => void) => {
      finishedCb = cb;
      return Promise.resolve(vi.fn());
    });

    const queryClient = new QueryClient();
    const invalidateSpy = vi.spyOn(queryClient, 'invalidateQueries');
    renderWithProvider(queryClient);

    await waitFor(() => expect(finishedCb).toBeDefined());

    finishedCb?.({ runId: 5, status: 'success' });

    await waitFor(() => {
      expect(invalidateSpy).toHaveBeenCalledWith({ queryKey: ['dashboard'] });
      expect(invalidateSpy).toHaveBeenCalledWith({ queryKey: ['results'] });
      expect(invalidateSpy).toHaveBeenCalledWith({ queryKey: ['runs'] });
    });
  });

  it('shows an error from a failed checkNow() call', async () => {
    checkNow.mockRejectedValue({ kind: 'validation', message: 'No accounts configured' });
    const user = userEvent.setup();
    const queryClient = new QueryClient();

    function ErrorConsumer() {
      const { error, triggerCheck } = useCheckRun();
      return (
        <div>
          <p>{error?.message ?? 'no error'}</p>
          <button type="button" onClick={() => void triggerCheck()}>
            Trigger
          </button>
        </div>
      );
    }

    render(
      <QueryClientProvider client={queryClient}>
        <CheckRunProvider>
          <ErrorConsumer />
        </CheckRunProvider>
      </QueryClientProvider>,
    );

    await user.click(screen.getByRole('button', { name: 'Trigger' }));

    expect(await screen.findByText('No accounts configured')).toBeInTheDocument();
  });
});
