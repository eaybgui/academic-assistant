import { useState } from 'react';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import Modal from '../../components/Modal';
import ErrorMessage from '../../components/ErrorMessage';
import Field from '../../components/forms/Field';
import TextInput from '../../components/forms/TextInput';
import Select from '../../components/forms/Select';
import { api, toCommandError } from '../../lib/api/client';
import { queryKeys } from '../../lib/api/queryKeys';
import { formatConnectionTestMessage } from '../../lib/format';
import type { AccountDto, CommandError, UniversityDto } from '../../lib/api/types';
import {
  accountToFormState,
  buildAccountInput,
  emptyAccountFormState,
  existingTenant,
  hasErrors,
  validateAccountForm,
  type AccountFormState,
  type ProviderType,
} from './accountForm';

export interface AccountFormModalProps {
  open: boolean;
  account: AccountDto | null;
  universities: UniversityDto[];
  onClose: () => void;
}

/**
 * Add/edit modal for an email account. The provider switch (IMAP / Microsoft 365) is only
 * editable when creating a new account. After a successful save of a Microsoft account it offers
 * a "Sign in with Microsoft" action.
 */
export default function AccountFormModal({ open, account, universities, onClose }: AccountFormModalProps) {
  const isCreate = account === null;
  const [state, setState] = useState<AccountFormState>(() =>
    account ? accountToFormState(account) : emptyAccountFormState(),
  );
  const [errors, setErrors] = useState<ReturnType<typeof validateAccountForm>>({});
  const [saveError, setSaveError] = useState<CommandError | null>(null);
  const [testResult, setTestResult] = useState<string | null>(null);
  const [savedAccount, setSavedAccount] = useState<AccountDto | null>(account);
  const [signedIn, setSignedIn] = useState(false);

  const queryClient = useQueryClient();

  function invalidateAfterSave() {
    void queryClient.invalidateQueries({ queryKey: queryKeys.accounts });
    void queryClient.invalidateQueries({ queryKey: ['dashboard'] });
  }

  const saveMutation = useMutation({
    mutationFn: () => {
      // Once a Microsoft account has been created it is saved (but not closed, so the "Sign in
      // with Microsoft" action can follow); a second Save must update that account instead of
      // creating a duplicate one, even though `account` (the modal's original "editing" prop)
      // is still null.
      const target = account ?? savedAccount;
      const tenant = existingTenant(target ?? undefined);
      const input = buildAccountInput(state, tenant);
      return target ? api.updateAccount(target.id, input) : api.addAccount(input);
    },
    onSuccess: (saved) => {
      setSaveError(null);
      setSavedAccount(saved);
      invalidateAfterSave();
      if (saved.provider.type === 'imap') {
        onClose();
      }
    },
    onError: (e) => setSaveError(toCommandError(e)),
  });

  const testMutation = useMutation({
    mutationFn: (id: number) => api.testAccount(id),
    onSuccess: (result) => setTestResult(formatConnectionTestMessage(result)),
    onError: (e) => setTestResult(toCommandError(e).message),
  });

  const signInMutation = useMutation({
    mutationFn: (id: number) => api.connectMicrosoftAccount(id),
    onSuccess: (saved) => {
      setSavedAccount(saved);
      setSaveError(null);
      setSignedIn(true);
      invalidateAfterSave();
    },
    onError: (e) => setSaveError(toCommandError(e)),
  });

  function handleProviderChange(providerType: ProviderType) {
    setState((prev) => ({
      ...emptyAccountFormState(providerType),
      name: prev.name,
      emailAddress: prev.emailAddress,
      defaultUniversityId: prev.defaultUniversityId,
      initialSyncDays: prev.initialSyncDays,
    }));
    setErrors({});
  }

  function handleSubmit() {
    const nextErrors = validateAccountForm(state, isCreate);
    setErrors(nextErrors);
    if (hasErrors(nextErrors)) return;
    saveMutation.mutate();
  }

  const universityOptions = [
    { value: '', label: 'None' },
    ...universities.map((u) => ({ value: String(u.id), label: u.name })),
  ];

  return (
    <Modal
      open={open}
      title={isCreate ? 'Add account' : 'Edit account'}
      onClose={onClose}
      footer={
        <>
          <button
            type="button"
            onClick={onClose}
            className="rounded border border-neutral-300 px-3 py-1.5 text-sm text-neutral-700 hover:bg-neutral-50"
          >
            Cancel
          </button>
          {!isCreate ? (
            <button
              type="button"
              disabled={testMutation.isPending}
              onClick={() => account && testMutation.mutate(account.id)}
              className="rounded border border-neutral-300 px-3 py-1.5 text-sm text-neutral-700 hover:bg-neutral-50 disabled:opacity-50"
            >
              {testMutation.isPending ? 'Testing…' : 'Test connection'}
            </button>
          ) : null}
          <button
            type="button"
            disabled={saveMutation.isPending}
            onClick={handleSubmit}
            className="rounded bg-neutral-900 px-3 py-1.5 text-sm font-medium text-white hover:bg-neutral-800 disabled:opacity-50"
          >
            {saveMutation.isPending ? 'Saving…' : 'Save'}
          </button>
        </>
      }
    >
      <div className="flex flex-col gap-4">
        <div className="flex rounded border border-neutral-300 p-0.5 text-sm">
          <button
            type="button"
            disabled={!isCreate}
            onClick={() => handleProviderChange('imap')}
            className={`flex-1 rounded px-3 py-1.5 ${
              state.providerType === 'imap' ? 'bg-neutral-900 text-white' : 'text-neutral-700'
            } disabled:cursor-not-allowed`}
          >
            IMAP
          </button>
          <button
            type="button"
            disabled={!isCreate}
            onClick={() => handleProviderChange('microsoft_graph')}
            className={`flex-1 rounded px-3 py-1.5 ${
              state.providerType === 'microsoft_graph' ? 'bg-neutral-900 text-white' : 'text-neutral-700'
            } disabled:cursor-not-allowed`}
          >
            Microsoft 365
          </button>
        </div>

        <div className="grid grid-cols-2 gap-4">
          <Field label="Display name" htmlFor="account-name" error={errors.name}>
            <TextInput
              id="account-name"
              value={state.name}
              onChange={(e) => setState({ ...state, name: e.target.value })}
            />
          </Field>
          <Field label="Email address" htmlFor="account-email" error={errors.emailAddress}>
            <TextInput
              id="account-email"
              placeholder="name@alumni.uv.es"
              value={state.emailAddress}
              onChange={(e) => setState({ ...state, emailAddress: e.target.value })}
            />
          </Field>

          {state.providerType === 'imap' ? (
            <>
              <Field label="IMAP host" htmlFor="account-host" error={errors.host}>
                <TextInput
                  id="account-host"
                  placeholder="imap.uv.es"
                  value={state.host}
                  onChange={(e) => setState({ ...state, host: e.target.value })}
                />
              </Field>
              <Field label="Port (TLS)" htmlFor="account-port" error={errors.port}>
                <TextInput
                  id="account-port"
                  inputMode="numeric"
                  value={state.port}
                  onChange={(e) => setState({ ...state, port: e.target.value })}
                />
              </Field>
              <Field label="Username" htmlFor="account-username" error={errors.username}>
                <TextInput
                  id="account-username"
                  value={state.username}
                  onChange={(e) => setState({ ...state, username: e.target.value })}
                />
              </Field>
              <Field
                label="Password"
                htmlFor="account-password"
                error={errors.password}
                hint="The password is saved in Windows Credential Manager, never in the database."
              >
                <TextInput
                  id="account-password"
                  type="password"
                  autoComplete="new-password"
                  value={state.password}
                  onChange={(e) => setState({ ...state, password: e.target.value })}
                />
              </Field>
            </>
          ) : (
            <p className="col-span-2 text-xs text-neutral-500">
              Your sign-in is saved in Windows Credential Manager, never in the database.
            </p>
          )}

          <Field label="Folders" htmlFor="account-folders" error={errors.folders}>
            <TextInput
              id="account-folders"
              placeholder="INBOX"
              value={state.folders}
              onChange={(e) => setState({ ...state, folders: e.target.value })}
            />
          </Field>
          <Field label="Default university" htmlFor="account-university">
            <Select
              id="account-university"
              options={universityOptions}
              value={state.defaultUniversityId}
              onChange={(e) => setState({ ...state, defaultUniversityId: e.target.value })}
            />
          </Field>
          <Field label="First sync: last days" htmlFor="account-sync-days" error={errors.initialSyncDays}>
            <TextInput
              id="account-sync-days"
              inputMode="numeric"
              value={state.initialSyncDays}
              onChange={(e) => setState({ ...state, initialSyncDays: e.target.value })}
            />
          </Field>
        </div>

        <ErrorMessage error={saveError} />

        {testResult ? <p className="text-sm text-neutral-700">{testResult}</p> : null}

        {state.providerType === 'microsoft_graph' && savedAccount ? (
          <div className="flex items-center gap-3 rounded border border-neutral-200 bg-neutral-50 px-3 py-2">
            <button
              type="button"
              disabled={signInMutation.isPending}
              onClick={() => {
                setSignedIn(false);
                signInMutation.mutate(savedAccount.id);
              }}
              className="rounded bg-neutral-900 px-3 py-1.5 text-sm font-medium text-white hover:bg-neutral-800 disabled:opacity-50"
            >
              Sign in with Microsoft
            </button>
            {signInMutation.isPending ? (
              <span className="text-sm text-neutral-600">Waiting for sign-in in your browser…</span>
            ) : signedIn ? (
              <span className="text-sm text-neutral-600">Signed in.</span>
            ) : null}
          </div>
        ) : null}
      </div>
    </Modal>
  );
}
