import { beforeEach, describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { MemoryRouter } from 'react-router';
import App from './App';
import type { SettingsDto, TrackerDto } from './lib/api/types';

const { getTracker, getSettings } = vi.hoisted(() => ({ getTracker: vi.fn(), getSettings: vi.fn() }));
const features = vi.hoisted(() => ({ EMAIL_FEATURES_ENABLED: false }));

vi.mock('./lib/api/client', async (importOriginal) => {
  const actual = await importOriginal<typeof import('./lib/api/client')>();
  return {
    ...actual,
    api: {
      getTracker,
      getSettings,
      getDashboard: vi.fn().mockRejectedValue(new Error('not needed')),
      getUniversities: vi.fn().mockRejectedValue(new Error('not needed')),
    },
  };
});
vi.mock('./config/features', () => features);
vi.mock('./features/sync/useSync', () => ({
  useSync: () => ({ running: false, progress: null, error: null, startSync: vi.fn() }),
}));

const settings: SettingsDto = {
  aiProvider: 'ollama',
  jevModel: 'jev-latest',
  jevApiKeyConfigured: false,
  ollamaBaseUrl: 'http://localhost:11434',
  ollamaModel: 'qwen3:8b',
  microsoftClientId: null,
  blockedSenders: [],
};

function renderAt(path: string) {
  return render(
    <QueryClientProvider client={new QueryClient()}>
      <MemoryRouter initialEntries={[path]}>
        <App />
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

beforeEach(() => {
  getTracker.mockReset().mockResolvedValue({ items: [], universities: [], personalTasks: [], syncRunning: false } satisfies TrackerDto);
  getSettings.mockReset().mockResolvedValue(settings);
});

describe('App routing', () => {
  it('redirects /settings to the dashboard when the email section is disabled', async () => {
    features.EMAIL_FEATURES_ENABLED = false;

    renderAt('/settings');

    expect(await screen.findByRole('heading', { name: 'Track your deliverables' })).toBeInTheDocument();
    expect(getSettings).not.toHaveBeenCalled();
  });

  it('serves /settings when the email section is enabled', async () => {
    features.EMAIL_FEATURES_ENABLED = true;

    renderAt('/settings');

    expect(await screen.findByRole('heading', { name: 'Settings' })).toBeInTheDocument();
    expect(getSettings).toHaveBeenCalled();
  });
});
