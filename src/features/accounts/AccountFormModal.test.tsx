import { describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import AccountFormModal from './AccountFormModal';
import { api } from '../../lib/api/client';
import type { AccountDto, AccountInput } from '../../lib/api/types';

vi.mock('../../lib/api/client', async (importOriginal) => {
  const actual = await importOriginal<typeof import('../../lib/api/client')>();
  return {
    ...actual,
    api: {
      addAccount: vi.fn(),
      updateAccount: vi.fn(),
      testAccount: vi.fn(),
      connectMicrosoftAccount: vi.fn(),
    },
  };
});

const mockedApi = vi.mocked(api, true);

const imapAccount: AccountDto = {
  id: 3,
  name: 'UV IMAP',
  emailAddress: 'me@uv.es',
  provider: { type: 'imap', host: 'imap.uv.es', port: 993, username: 'me@uv.es' },
  folders: ['INBOX'],
  defaultUniversityId: null,
  initialSyncDays: 14,
  enabled: true,
  credentialsConfigured: true,
  lastSync: { at: null, status: 'never', error: null },
  createdAt: '2026-09-28T00:00:00Z',
  updatedAt: '2026-09-28T00:00:00Z',
};

function renderModal(account: AccountDto | null = null) {
  const queryClient = new QueryClient();
  return render(
    <QueryClientProvider client={queryClient}>
      <AccountFormModal open account={account} universities={[]} onClose={() => {}} />
    </QueryClientProvider>,
  );
}

describe('AccountFormModal', () => {
  it('shows validation errors for an empty IMAP form and does not call addAccount', () => {
    renderModal();

    fireEvent.click(screen.getByRole('button', { name: 'Save' }));

    expect(screen.getByText('Display name is required.')).toBeInTheDocument();
    expect(screen.getByText('Enter a valid email address.')).toBeInTheDocument();
    expect(screen.getByText('IMAP host is required.')).toBeInTheDocument();
    expect(screen.getByText('Username is required.')).toBeInTheDocument();
    expect(screen.getByText('Password is required.')).toBeInTheDocument();
    expect(mockedApi.addAccount).not.toHaveBeenCalled();
  });

  it('calls addAccount with the exact IMAP payload on a valid submit', async () => {
    mockedApi.addAccount.mockResolvedValue({
      id: 1,
      name: 'UV',
      emailAddress: 'me@uv.es',
      provider: { type: 'imap', host: 'imap.uv.es', port: 993, username: 'me@uv.es' },
      folders: ['INBOX'],
      defaultUniversityId: null,
      initialSyncDays: 14,
      enabled: true,
      credentialsConfigured: true,
      lastSync: { at: null, status: 'never', error: null },
      createdAt: '2026-09-28T00:00:00Z',
      updatedAt: '2026-09-28T00:00:00Z',
    });

    renderModal();

    fireEvent.change(screen.getByLabelText('Display name'), { target: { value: 'UV' } });
    fireEvent.change(screen.getByLabelText('Email address'), { target: { value: 'me@uv.es' } });
    fireEvent.change(screen.getByLabelText('IMAP host'), { target: { value: 'imap.uv.es' } });
    fireEvent.change(screen.getByLabelText('Username'), { target: { value: 'me@uv.es' } });
    fireEvent.change(screen.getByLabelText('Password'), { target: { value: 'hunter2' } });

    fireEvent.click(screen.getByRole('button', { name: 'Save' }));

    const expected: AccountInput = {
      name: 'UV',
      emailAddress: 'me@uv.es',
      folders: ['INBOX'],
      defaultUniversityId: null,
      initialSyncDays: 14,
      enabled: true,
      provider: { type: 'imap', host: 'imap.uv.es', port: 993, username: 'me@uv.es', password: 'hunter2' },
    };

    await waitFor(() => expect(mockedApi.addAccount).toHaveBeenCalledWith(expected));
  });

  it('calls addAccount with tenant "organizations" and folders ["inbox"] for a Microsoft account', async () => {
    mockedApi.addAccount.mockResolvedValue({
      id: 2,
      name: 'UAH',
      emailAddress: 'me@uah.es',
      provider: { type: 'microsoft_graph', tenant: 'organizations' },
      folders: ['inbox'],
      defaultUniversityId: null,
      initialSyncDays: 14,
      enabled: true,
      credentialsConfigured: false,
      lastSync: { at: null, status: 'never', error: null },
      createdAt: '2026-09-28T00:00:00Z',
      updatedAt: '2026-09-28T00:00:00Z',
    });

    renderModal();

    fireEvent.click(screen.getByRole('button', { name: 'Microsoft 365' }));

    fireEvent.change(screen.getByLabelText('Display name'), { target: { value: 'UAH' } });
    fireEvent.change(screen.getByLabelText('Email address'), { target: { value: 'me@uah.es' } });

    fireEvent.click(screen.getByRole('button', { name: 'Save' }));

    await waitFor(() =>
      expect(mockedApi.addAccount).toHaveBeenCalledWith(
        expect.objectContaining({
          folders: ['inbox'],
          provider: { type: 'microsoft_graph', tenant: 'organizations' },
        }),
      ),
    );
  });

  it('prefills the form with the account being edited and sends the edited payload via updateAccount', async () => {
    mockedApi.updateAccount.mockResolvedValue(imapAccount);

    renderModal(imapAccount);

    expect(screen.getByLabelText('Display name')).toHaveValue('UV IMAP');
    expect(screen.getByLabelText('Email address')).toHaveValue('me@uv.es');
    expect(screen.getByLabelText('IMAP host')).toHaveValue('imap.uv.es');
    expect(screen.getByLabelText('Port (TLS)')).toHaveValue('993');
    expect(screen.getByLabelText('Username')).toHaveValue('me@uv.es');
    expect(screen.getByLabelText('Password')).toHaveValue('');

    fireEvent.change(screen.getByLabelText('Display name'), { target: { value: 'UV IMAP renamed' } });
    fireEvent.click(screen.getByRole('button', { name: 'Save' }));

    await waitFor(() =>
      expect(mockedApi.updateAccount).toHaveBeenCalledWith(
        3,
        expect.objectContaining({ name: 'UV IMAP renamed', emailAddress: 'me@uv.es' }),
      ),
    );
  });

  it('does not create a duplicate Microsoft account on a second Save', async () => {
    const created = {
      id: 9,
      name: 'UAH',
      emailAddress: 'me@uah.es',
      provider: { type: 'microsoft_graph' as const, tenant: 'organizations' },
      folders: ['inbox'],
      defaultUniversityId: null,
      initialSyncDays: 14,
      enabled: true,
      credentialsConfigured: false,
      lastSync: { at: null, status: 'never' as const, error: null },
      createdAt: '2026-09-28T00:00:00Z',
      updatedAt: '2026-09-28T00:00:00Z',
    };
    mockedApi.addAccount.mockResolvedValue(created);
    mockedApi.updateAccount.mockResolvedValue(created);

    renderModal();

    fireEvent.click(screen.getByRole('button', { name: 'Microsoft 365' }));
    fireEvent.change(screen.getByLabelText('Display name'), { target: { value: 'UAH' } });
    fireEvent.change(screen.getByLabelText('Email address'), { target: { value: 'me@uah.es' } });

    fireEvent.click(screen.getByRole('button', { name: 'Save' }));
    await waitFor(() => expect(mockedApi.addAccount).toHaveBeenCalledTimes(1));

    fireEvent.click(screen.getByRole('button', { name: 'Save' }));
    await waitFor(() => expect(mockedApi.updateAccount).toHaveBeenCalledWith(9, expect.anything()));

    expect(mockedApi.addAccount).toHaveBeenCalledTimes(1);
  });
});
