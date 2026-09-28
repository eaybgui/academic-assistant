import type { AccountDto, AccountInput } from '../../lib/api/types';

export type ProviderType = 'imap' | 'microsoft_graph';

export const DEFAULT_IMAP_PORT = '993';
export const DEFAULT_IMAP_FOLDERS = 'INBOX';
export const DEFAULT_GRAPH_FOLDERS = 'inbox';
export const DEFAULT_INITIAL_SYNC_DAYS = '14';
const DEFAULT_GRAPH_TENANT = 'organizations';

/** Editable state behind the account add/edit form. Numeric fields stay as text while being edited. */
export interface AccountFormState {
  providerType: ProviderType;
  name: string;
  emailAddress: string;
  host: string;
  port: string;
  username: string;
  password: string;
  folders: string;
  defaultUniversityId: string;
  initialSyncDays: string;
  enabled: boolean;
}

export function emptyAccountFormState(providerType: ProviderType = 'imap'): AccountFormState {
  return {
    providerType,
    name: '',
    emailAddress: '',
    host: '',
    port: DEFAULT_IMAP_PORT,
    username: '',
    password: '',
    folders: providerType === 'imap' ? DEFAULT_IMAP_FOLDERS : DEFAULT_GRAPH_FOLDERS,
    defaultUniversityId: '',
    initialSyncDays: DEFAULT_INITIAL_SYNC_DAYS,
    enabled: true,
  };
}

/** Builds the editable form state from an existing account. The password field always starts empty. */
export function accountToFormState(account: AccountDto): AccountFormState {
  return {
    providerType: account.provider.type,
    name: account.name,
    emailAddress: account.emailAddress,
    host: account.provider.type === 'imap' ? account.provider.host : '',
    port: account.provider.type === 'imap' ? String(account.provider.port) : DEFAULT_IMAP_PORT,
    username: account.provider.type === 'imap' ? account.provider.username : '',
    password: '',
    folders: account.folders.join(', '),
    defaultUniversityId:
      account.defaultUniversityId != null ? String(account.defaultUniversityId) : '',
    initialSyncDays: String(account.initialSyncDays),
    enabled: account.enabled,
  };
}

/** The Microsoft tenant carried by an existing account, kept unchanged across edits (never shown in the form). */
export function existingTenant(account: AccountDto | undefined): string | undefined {
  return account && account.provider.type === 'microsoft_graph' ? account.provider.tenant : undefined;
}

export type AccountFormErrors = Partial<
  Record<
    'name' | 'emailAddress' | 'host' | 'port' | 'username' | 'password' | 'folders' | 'initialSyncDays',
    string
  >
>;

function parseFolders(raw: string): string[] {
  return raw
    .split(',')
    .map((f) => f.trim())
    .filter((f) => f.length > 0);
}

export function validateAccountForm(state: AccountFormState, isCreate: boolean): AccountFormErrors {
  const errors: AccountFormErrors = {};

  if (!state.name.trim()) errors.name = 'Display name is required.';
  if (!state.emailAddress.trim().includes('@')) errors.emailAddress = 'Enter a valid email address.';
  if (parseFolders(state.folders).length === 0) errors.folders = 'Enter at least one folder.';

  const days = Number(state.initialSyncDays);
  if (!Number.isInteger(days) || days < 1 || days > 365) {
    errors.initialSyncDays = 'Enter a number of days between 1 and 365.';
  }

  if (state.providerType === 'imap') {
    if (!state.host.trim()) errors.host = 'IMAP host is required.';

    const port = Number(state.port);
    if (!Number.isInteger(port) || port < 1 || port > 65535) {
      errors.port = 'Enter a port between 1 and 65535.';
    }

    if (!state.username.trim()) errors.username = 'Username is required.';
    if (isCreate && !state.password) errors.password = 'Password is required.';
  }

  return errors;
}

export function hasErrors(errors: AccountFormErrors): boolean {
  return Object.keys(errors).length > 0;
}

/**
 * Builds the exact `AccountInput` payload for `addAccount`/`updateAccount`. The IMAP password is
 * only included when the user typed one (required by validation on create; optional on edit —
 * omitting it keeps the stored password unchanged). The Microsoft tenant is never editable: it
 * keeps the value the account already has, defaulting to `"organizations"` for a new account.
 */
export function buildAccountInput(state: AccountFormState, tenant?: string): AccountInput {
  const provider: AccountInput['provider'] =
    state.providerType === 'imap'
      ? {
          type: 'imap',
          host: state.host.trim(),
          port: Number(state.port),
          username: state.username.trim(),
          ...(state.password ? { password: state.password } : {}),
        }
      : { type: 'microsoft_graph', tenant: tenant ?? DEFAULT_GRAPH_TENANT };

  return {
    name: state.name.trim(),
    emailAddress: state.emailAddress.trim(),
    folders: parseFolders(state.folders),
    defaultUniversityId: state.defaultUniversityId ? Number(state.defaultUniversityId) : null,
    initialSyncDays: Number(state.initialSyncDays),
    enabled: state.enabled,
    provider,
  };
}
