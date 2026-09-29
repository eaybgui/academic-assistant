import { describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import AccountsPage from './AccountsPage';
import { api } from '../../lib/api/client';
import type { AccountDto } from '../../lib/api/types';

vi.mock('../../lib/api/client', async (importOriginal) => {
  const actual = await importOriginal<typeof import('../../lib/api/client')>();
  return {
    ...actual,
    api: {
      getAccounts: vi.fn(),
      getCatalog: vi.fn(),
      updateAccount: vi.fn(),
      removeAccount: vi.fn(),
      testAccount: vi.fn(),
      connectMicrosoftAccount: vi.fn(),
      addAccount: vi.fn(),
    },
  };
});

const mockedApi = vi.mocked(api, true);

const imapAccount: AccountDto = {
  id: 1,
  name: 'UV IMAP',
  emailAddress: 'me@uv.es',
  provider: { type: 'imap', host: 'imap.uv.es', port: 993, username: 'me@uv.es' },
  folders: ['INBOX'],
  defaultUniversityId: null,
  initialSyncDays: 14,
  enabled: true,
  credentialsConfigured: true,
  lastSync: { at: '2026-09-28T08:00:00Z', status: 'ok', error: null },
  createdAt: '2026-09-01T00:00:00Z',
  updatedAt: '2026-09-01T00:00:00Z',
};

function renderPage() {
  const queryClient = new QueryClient();
  return render(
    <QueryClientProvider client={queryClient}>
      <AccountsPage />
    </QueryClientProvider>,
  );
}

describe('AccountsPage', () => {
  it('toggles enabled off via updateAccount', async () => {
    mockedApi.getAccounts.mockResolvedValue([imapAccount]);
    mockedApi.getCatalog.mockResolvedValue([]);
    mockedApi.updateAccount.mockResolvedValue({ ...imapAccount, enabled: false });

    renderPage();

    const toggle = await screen.findByRole('switch', { name: 'Enabled' });
    fireEvent.click(toggle);

    await waitFor(() =>
      expect(mockedApi.updateAccount).toHaveBeenCalledWith(
        1,
        expect.objectContaining({ enabled: false, name: 'UV IMAP' }),
      ),
    );
  });

  it('requires confirmation before deleting an account', async () => {
    mockedApi.getAccounts.mockResolvedValue([imapAccount]);
    mockedApi.getCatalog.mockResolvedValue([]);

    renderPage();

    const deleteButton = await screen.findByRole('button', { name: 'Delete UV IMAP' });
    fireEvent.click(deleteButton);

    expect(
      screen.getByText('Deletes this account and all its results. Disable it instead to keep its history.'),
    ).toBeInTheDocument();
    expect(mockedApi.removeAccount).not.toHaveBeenCalled();

    fireEvent.click(screen.getByRole('button', { name: 'Delete' }));

    await waitFor(() => expect(mockedApi.removeAccount).toHaveBeenCalledWith(1));
  });

  it('shows a delete failure inside the confirmation dialog', async () => {
    mockedApi.getAccounts.mockResolvedValue([imapAccount]);
    mockedApi.getCatalog.mockResolvedValue([]);
    mockedApi.removeAccount.mockRejectedValue({ kind: 'internal', message: 'Could not delete the account' });

    renderPage();

    fireEvent.click(await screen.findByRole('button', { name: 'Delete UV IMAP' }));
    fireEvent.click(screen.getByRole('button', { name: 'Delete' }));

    const dialog = screen.getByRole('dialog');
    expect(await within(dialog).findByText('Could not delete the account')).toBeInTheDocument();
  });

  it('shows the test connection result inline', async () => {
    mockedApi.getAccounts.mockResolvedValue([imapAccount]);
    mockedApi.getCatalog.mockResolvedValue([]);
    mockedApi.testAccount.mockResolvedValue({ ok: false, errorKind: 'auth', message: 'Login rejected' });

    renderPage();

    const testButton = await screen.findByRole('button', { name: 'Test' });
    fireEvent.click(testButton);

    expect(await screen.findByText('Login rejected')).toBeInTheDocument();
  });

  it('discards unsaved edits when the edit modal is cancelled and reopened', async () => {
    mockedApi.getAccounts.mockResolvedValue([imapAccount]);
    mockedApi.getCatalog.mockResolvedValue([]);

    renderPage();

    fireEvent.click(await screen.findByRole('button', { name: 'Edit' }));
    fireEvent.change(screen.getByLabelText('Display name'), { target: { value: 'Unsaved name' } });
    expect(screen.getByLabelText('Display name')).toHaveValue('Unsaved name');

    fireEvent.click(screen.getByRole('button', { name: 'Cancel' }));
    fireEvent.click(screen.getByRole('button', { name: 'Edit' }));

    expect(screen.getByLabelText('Display name')).toHaveValue('UV IMAP');
  });
});
