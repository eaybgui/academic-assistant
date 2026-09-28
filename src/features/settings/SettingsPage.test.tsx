import { describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import SettingsPage from './SettingsPage';
import { api } from '../../lib/api/client';
import type { SettingsDto } from '../../lib/api/types';

vi.mock('../../lib/api/client', async (importOriginal) => {
  const actual = await importOriginal<typeof import('../../lib/api/client')>();
  return {
    ...actual,
    api: {
      getSettings: vi.fn(),
      updateSettings: vi.fn(),
      setJevApiKey: vi.fn(),
      clearJevApiKey: vi.fn(),
      testAiProvider: vi.fn(),
    },
  };
});

const mockedApi = vi.mocked(api, true);

const settings: SettingsDto = {
  aiProvider: 'ollama',
  jevModel: 'jev-latest',
  jevApiKeyConfigured: false,
  ollamaBaseUrl: 'http://localhost:11434',
  ollamaModel: 'qwen3:8b',
  microsoftClientId: null,
  blockedSenders: ['noreply@example.com'],
};

function renderPage() {
  const queryClient = new QueryClient();
  return render(
    <QueryClientProvider client={queryClient}>
      <SettingsPage />
    </QueryClientProvider>,
  );
}

describe('SettingsPage', () => {
  it('sends the blocked senders array on save', async () => {
    mockedApi.getSettings.mockResolvedValue(settings);
    mockedApi.updateSettings.mockResolvedValue(settings);

    renderPage();

    const textarea = await screen.findByLabelText('Blocked senders and domains (one per line)');
    fireEvent.change(textarea, { target: { value: 'noreply@example.com\nspam@example.com\n\n  ' } });

    fireEvent.click(screen.getByRole('button', { name: 'Save' }));

    await waitFor(() =>
      expect(mockedApi.updateSettings).toHaveBeenCalledWith(
        expect.objectContaining({ blockedSenders: ['noreply@example.com', 'spam@example.com'] }),
      ),
    );
  });

  it('replaces the jev key and never renders it afterwards', async () => {
    mockedApi.getSettings.mockResolvedValue(settings);
    mockedApi.setJevApiKey.mockResolvedValue(undefined);

    renderPage();

    fireEvent.click(await screen.findByRole('button', { name: 'Replace key' }));
    const dialog = screen.getByRole('dialog');
    const keyInput = within(dialog).getByLabelText('API key');
    fireEvent.change(keyInput, { target: { value: 'super-secret-key' } });
    fireEvent.click(within(dialog).getByRole('button', { name: 'Save' }));

    await waitFor(() => expect(mockedApi.setJevApiKey).toHaveBeenCalledWith('super-secret-key'));
    expect(screen.queryByDisplayValue('super-secret-key')).not.toBeInTheDocument();
    expect(screen.queryByText('super-secret-key')).not.toBeInTheDocument();
  });

  it('shows the error message from a failed provider test', async () => {
    mockedApi.getSettings.mockResolvedValue(settings);
    mockedApi.testAiProvider.mockResolvedValue({ ok: false, errorKind: 'connection', message: 'Ollama is not running' });

    renderPage();

    fireEvent.click(await screen.findByRole('button', { name: 'Test provider' }));

    expect(await screen.findByText('Ollama is not running')).toBeInTheDocument();
  });
});
