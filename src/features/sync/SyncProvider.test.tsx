import { beforeEach, describe, expect, it, vi } from 'vitest';
import { act, render, renderHook, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { SyncProvider } from './SyncProvider';
import { useSync } from './useSync';
import type { LmsSyncFinishedEvent, LmsSyncProgressEvent, LmsSyncResultDto } from '../../lib/api/types';

const { lmsSync } = vi.hoisted(() => ({ lmsSync: vi.fn() }));

vi.mock('../../lib/api/client', async (importOriginal) => {
  const actual = await importOriginal<typeof import('../../lib/api/client')>();
  return { ...actual, api: { ...actual.api, lmsSync } };
});

const { onLmsSyncProgress, onLmsSyncFinished } = vi.hoisted(() => ({
  onLmsSyncProgress: vi.fn(),
  onLmsSyncFinished: vi.fn(),
}));

vi.mock('../../lib/api/events', () => ({ onLmsSyncProgress, onLmsSyncFinished }));

let progressCb: ((e: LmsSyncProgressEvent) => void) | undefined;
let finishedCb: ((e: LmsSyncFinishedEvent) => void) | undefined;

const result: LmsSyncResultDto = {
  universityId: 1,
  universityName: 'UV',
  outcome: 'success',
  newItems: 2,
  dueChanged: 0,
  removed: 0,
  error: null,
};

/** A minimal consumer standing in for "any route". */
function TestConsumer() {
  const { running, progress, lastResults, error, startSync } = useSync();
  return (
    <div>
      <p>{running ? `running:${progress ?? ''}` : 'idle'}</p>
      <p>{error ? `error:${error.message}` : 'no error'}</p>
      <p>{lastResults ? `results:${lastResults.length}` : 'no results'}</p>
      <button type="button" onClick={() => void startSync(1)}>
        Sync
      </button>
    </div>
  );
}

function renderWithProvider(queryClient: QueryClient) {
  return render(
    <QueryClientProvider client={queryClient}>
      <SyncProvider>
        <TestConsumer />
      </SyncProvider>
    </QueryClientProvider>,
  );
}

beforeEach(() => {
  progressCb = undefined;
  finishedCb = undefined;
  lmsSync.mockReset().mockResolvedValue(undefined);
  onLmsSyncProgress.mockReset().mockImplementation((cb: (e: LmsSyncProgressEvent) => void) => {
    progressCb = cb;
    return Promise.resolve(vi.fn());
  });
  onLmsSyncFinished.mockReset().mockImplementation((cb: (e: LmsSyncFinishedEvent) => void) => {
    finishedCb = cb;
    return Promise.resolve(vi.fn());
  });
});

describe('SyncProvider', () => {
  it('runs from startSync until lms-sync:finished, stores the results and invalidates the queries', async () => {
    const queryClient = new QueryClient();
    const invalidateSpy = vi.spyOn(queryClient, 'invalidateQueries');
    const user = userEvent.setup();
    renderWithProvider(queryClient);
    await waitFor(() => expect(finishedCb).toBeDefined());

    await user.click(screen.getByRole('button', { name: 'Sync' }));
    expect(lmsSync).toHaveBeenCalledWith(1);
    expect(await screen.findByText(/^running:/)).toBeInTheDocument();

    act(() => finishedCb?.({ results: [result] }));

    expect(await screen.findByText('idle')).toBeInTheDocument();
    expect(screen.getByText('results:1')).toBeInTheDocument();
    expect(invalidateSpy).toHaveBeenCalledWith({ queryKey: ['tracker'] });
    expect(invalidateSpy).toHaveBeenCalledWith({ queryKey: ['universities'] });
    expect(invalidateSpy).toHaveBeenCalledWith({ queryKey: ['courses'] });
  });

  it('does not stay running when lms-sync:finished arrives before startSync() resolves', async () => {
    let resolveSync: () => void = () => {};
    lmsSync.mockImplementation(
      () =>
        new Promise<void>((resolve) => {
          resolveSync = resolve;
        }),
    );
    const user = userEvent.setup();
    renderWithProvider(new QueryClient());
    await waitFor(() => expect(finishedCb).toBeDefined());

    await user.click(screen.getByRole('button', { name: 'Sync' }));
    await waitFor(() => expect(lmsSync).toHaveBeenCalled());

    act(() => finishedCb?.({ results: [result] }));
    await act(async () => resolveSync());

    expect(screen.getByText('idle')).toBeInTheDocument();
    expect(screen.getByText('results:1')).toBeInTheDocument();
  });

  it('exposes the lastResults payload as received', async () => {
    const queryClient = new QueryClient();
    const wrapper = ({ children }: { children: React.ReactNode }) => (
      <QueryClientProvider client={queryClient}>
        <SyncProvider>{children}</SyncProvider>
      </QueryClientProvider>
    );
    const { result: hook } = renderHook(() => useSync(), { wrapper });
    await waitFor(() => expect(finishedCb).toBeDefined());
    expect(hook.current.lastResults).toBeNull();

    act(() => finishedCb?.({ results: [result] }));

    expect(hook.current.lastResults).toEqual([result]);
    act(() => hook.current.clearResults());
    expect(hook.current.lastResults).toBeNull();
  });

  it('shows the latest progress event while running', async () => {
    const user = userEvent.setup();
    renderWithProvider(new QueryClient());
    await waitFor(() => expect(progressCb).toBeDefined());

    await user.click(screen.getByRole('button', { name: 'Sync' }));
    act(() => progressCb?.({ universityId: 1, universityName: 'UV', stage: 'items' }));

    expect(await screen.findByText('running:UV: Fetching items')).toBeInTheDocument();
  });

  it('shows the error of a rejected startSync and stays idle', async () => {
    lmsSync.mockRejectedValue({ kind: 'conflict', message: 'A sync is already running' });
    const user = userEvent.setup();
    renderWithProvider(new QueryClient());

    await user.click(screen.getByRole('button', { name: 'Sync' }));

    expect(await screen.findByText('error:A sync is already running')).toBeInTheDocument();
    expect(screen.getByText('idle')).toBeInTheDocument();
  });

  it('clears a stale sync error when a sync finishes', async () => {
    lmsSync.mockRejectedValue({ kind: 'conflict', message: 'A sync is already running' });
    const user = userEvent.setup();
    renderWithProvider(new QueryClient());
    await waitFor(() => expect(finishedCb).toBeDefined());

    await user.click(screen.getByRole('button', { name: 'Sync' }));
    expect(await screen.findByText('error:A sync is already running')).toBeInTheDocument();

    act(() => finishedCb?.({ results: [result] }));

    expect(await screen.findByText('no error')).toBeInTheDocument();
  });

  it('marks the sync as running when a progress event arrives without a prior startSync', async () => {
    renderWithProvider(new QueryClient());
    await waitFor(() => expect(progressCb).toBeDefined());

    act(() => progressCb?.({ universityId: 2, universityName: 'UV', stage: 'courses' }));

    expect(await screen.findByText('running:UV: Checking courses')).toBeInTheDocument();
  });

  it('stops listening on unmount, including a subscription that resolves afterwards', async () => {
    const unlistenProgress = vi.fn();
    const unlistenFinished = vi.fn();
    onLmsSyncProgress.mockResolvedValue(unlistenProgress);
    onLmsSyncFinished.mockResolvedValue(unlistenFinished);

    const { unmount } = renderWithProvider(new QueryClient());
    unmount();

    await waitFor(() => {
      expect(unlistenProgress).toHaveBeenCalledTimes(1);
      expect(unlistenFinished).toHaveBeenCalledTimes(1);
    });
  });
});

describe('useSync', () => {
  it('throws outside a SyncProvider', () => {
    const spy = vi.spyOn(console, 'error').mockImplementation(() => {});
    expect(() => renderHook(() => useSync())).toThrow('useSync must be used within a SyncProvider');
    spy.mockRestore();
  });
});
