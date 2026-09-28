import { useState } from 'react';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { api, toCommandError } from '../../lib/api/client';
import { queryKeys } from '../../lib/api/queryKeys';
import ErrorMessage from '../../components/ErrorMessage';
import ConfirmDialog from '../../components/forms/ConfirmDialog';
import Toggle from '../../components/forms/Toggle';
import { TrashIcon } from '../../components/forms/icons';
import type { AccountDto, CommandError } from '../../lib/api/types';
import AccountFormModal from './AccountFormModal';
import { accountToFormState, buildAccountInput } from './accountForm';

function formatLastSync(account: AccountDto): { text: string; isError: boolean } {
  if (account.provider.type === 'microsoft_graph' && !account.credentialsConfigured) {
    return { text: 'Sign-in required', isError: true };
  }
  if (account.lastSync.status === 'error') {
    return { text: account.lastSync.error ?? 'Sync failed', isError: true };
  }
  if (account.lastSync.status === 'ok' && account.lastSync.at) {
    return { text: `last sync ${new Date(account.lastSync.at).toLocaleString()} OK`, isError: false };
  }
  return { text: 'never synced', isError: false };
}

/** Accounts screen: list of email accounts plus the add/edit modal and delete confirmation. */
export default function AccountsPage() {
  const queryClient = useQueryClient();
  const accountsQuery = useQuery({ queryKey: queryKeys.accounts, queryFn: api.getAccounts });
  const catalogQuery = useQuery({ queryKey: queryKeys.catalog, queryFn: api.getCatalog });

  const [modalOpen, setModalOpen] = useState(false);
  const [modalAccount, setModalAccount] = useState<AccountDto | null>(null);
  const [deleteTarget, setDeleteTarget] = useState<AccountDto | null>(null);
  const [testResults, setTestResults] = useState<Record<number, string>>({});
  const [rowError, setRowError] = useState<CommandError | null>(null);

  const universities = catalogQuery.data ?? [];
  const universityName = (id: number | null) => universities.find((u) => u.id === id)?.name ?? null;

  function invalidateAll() {
    void queryClient.invalidateQueries({ queryKey: queryKeys.accounts });
    void queryClient.invalidateQueries({ queryKey: ['dashboard'] });
  }

  const toggleMutation = useMutation({
    mutationFn: (account: AccountDto) => {
      const state = accountToFormState(account);
      state.enabled = !account.enabled;
      const tenant = account.provider.type === 'microsoft_graph' ? account.provider.tenant : undefined;
      return api.updateAccount(account.id, buildAccountInput(state, tenant));
    },
    onSuccess: invalidateAll,
    onError: (e) => setRowError(toCommandError(e)),
  });

  const testMutation = useMutation({
    mutationFn: (id: number) => api.testAccount(id),
    onSuccess: (result, id) =>
      setTestResults((prev) => ({
        ...prev,
        [id]: result.ok ? 'Connection successful.' : (result.message ?? 'Connection failed.'),
      })),
    onError: (e, id) => setTestResults((prev) => ({ ...prev, [id]: toCommandError(e).message })),
  });

  const signInMutation = useMutation({
    mutationFn: (id: number) => api.connectMicrosoftAccount(id),
    onSuccess: invalidateAll,
    onError: (e) => setRowError(toCommandError(e)),
  });

  const deleteMutation = useMutation({
    mutationFn: (id: number) => api.removeAccount(id),
    onSuccess: () => {
      setDeleteTarget(null);
      invalidateAll();
    },
    onError: (e) => setRowError(toCommandError(e)),
  });

  function openAddModal() {
    setModalAccount(null);
    setModalOpen(true);
  }

  function openEditModal(account: AccountDto) {
    setModalAccount(account);
    setModalOpen(true);
  }

  function closeModal() {
    setModalOpen(false);
    setModalAccount(null);
  }

  return (
    <div className="flex flex-col gap-4">
      <div className="flex items-center justify-between">
        <h1 className="text-2xl font-semibold text-neutral-900">Accounts</h1>
        <button
          type="button"
          onClick={openAddModal}
          className="rounded bg-neutral-900 px-3 py-1.5 text-sm font-medium text-white hover:bg-neutral-800"
        >
          Add account
        </button>
      </div>

      <ErrorMessage error={rowError} />

      {accountsQuery.isLoading ? <p className="text-sm text-neutral-500">Loading…</p> : null}
      {accountsQuery.data && accountsQuery.data.length === 0 ? (
        <p className="text-sm text-neutral-500">No accounts yet. Add one to start syncing email.</p>
      ) : null}

      <ul className="flex flex-col gap-2">
        {(accountsQuery.data ?? []).map((account) => {
          const sync = formatLastSync(account);
          const providerLabel = account.provider.type === 'imap' ? 'IMAP' : 'Microsoft 365';
          const uniName = universityName(account.defaultUniversityId) ?? 'No university';
          const isMicrosoft = account.provider.type === 'microsoft_graph';
          const isSigningIn = signInMutation.isPending && signInMutation.variables === account.id;

          return (
            <li key={account.id} className="rounded border border-neutral-200 bg-white px-4 py-3">
              <div className="flex items-center justify-between gap-4">
                <div className="flex items-start gap-3">
                  <span className="mt-0.5 flex h-8 w-8 shrink-0 items-center justify-center rounded-full bg-neutral-100 text-sm font-semibold text-neutral-600">
                    {account.name.slice(0, 1).toUpperCase() || '?'}
                  </span>
                  <div>
                    <p className="font-bold text-neutral-900">{account.name}</p>
                    <p className="text-sm text-neutral-500">
                      {providerLabel} · {account.emailAddress} · {uniName} · {account.folders.join(', ')} ·{' '}
                      <span className={sync.isError ? 'text-red-600' : ''}>{sync.text}</span>
                    </p>
                    {testResults[account.id] ? (
                      <p className="text-sm text-neutral-600">{testResults[account.id]}</p>
                    ) : null}
                    {isSigningIn ? (
                      <p className="text-sm text-neutral-600">Waiting for sign-in in your browser…</p>
                    ) : null}
                  </div>
                </div>

                <div className="flex items-center gap-3">
                  <button
                    type="button"
                    onClick={() => testMutation.mutate(account.id)}
                    disabled={testMutation.isPending}
                    className="text-sm text-neutral-700 hover:underline"
                  >
                    Test
                  </button>
                  <button
                    type="button"
                    onClick={() => openEditModal(account)}
                    className="text-sm text-neutral-700 hover:underline"
                  >
                    Edit
                  </button>
                  {isMicrosoft ? (
                    <button
                      type="button"
                      onClick={() => signInMutation.mutate(account.id)}
                      disabled={signInMutation.isPending}
                      className="text-sm text-neutral-700 hover:underline"
                    >
                      {account.credentialsConfigured ? 'Reconnect' : 'Sign in'}
                    </button>
                  ) : null}
                  <Toggle
                    checked={account.enabled}
                    onChange={() => toggleMutation.mutate(account)}
                    label="Enabled"
                  />
                  <button
                    type="button"
                    aria-label={`Delete ${account.name}`}
                    onClick={() => setDeleteTarget(account)}
                    className="text-neutral-400 hover:text-red-600"
                  >
                    <TrashIcon />
                  </button>
                </div>
              </div>
            </li>
          );
        })}
      </ul>

      <AccountFormModal
        open={modalOpen}
        account={modalAccount}
        universities={universities}
        onClose={closeModal}
      />

      <ConfirmDialog
        open={deleteTarget !== null}
        title="Delete account"
        message="Deletes this account and all its results. Disable it instead to keep its history."
        onCancel={() => setDeleteTarget(null)}
        onConfirm={() => deleteTarget && deleteMutation.mutate(deleteTarget.id)}
      />
    </div>
  );
}
