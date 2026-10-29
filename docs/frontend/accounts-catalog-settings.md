# Email accounts and Settings screens

Covers `src/features/accounts/**`, `src/features/settings/**` and the shared form primitives in
`src/components/forms/**`. These screens configure the email side of the app (mailboxes and global
settings); they don't fetch results or trigger a "Check now" run (see
[dashboard-results-runs.md](dashboard-results-runs.md) for that). The manual university/course
catalog screen no longer exists: universities and courses come from the learning platforms and are
managed on the Universities page ([tracker-universities.md](tracker-universities.md)). The file
name is kept for existing links.

## Shared form primitives (`src/components/forms/`)

- `Field` — label + control + optional hint/validation error, associated via `htmlFor`/`id`.
- `TextInput` / `Textarea` / `Select` — thin styled wrappers over the native elements, forwarding
  every native prop (`Select` takes an `options: { value, label }[]` list instead of children).
- `Toggle` — an accessible on/off switch (`role="switch"`, `aria-checked`) used for the account
  "Enabled" control.
- `ConfirmDialog` — a `Modal` wrapper for destructive-action confirmations (delete an account or a
  university). Takes `error` (rendered inside the dialog, since the page behind an open
  modal is covered) and `pending` (disables both buttons; the confirm button reads "Deleting…").
  The confirm label is always "Delete".
- `icons.tsx` — small inline SVG icons (`TrashIcon`, `PencilIcon`) used instead of emoji.

The screens also use the shared shell and API layer as they are (`Layout`, `Modal`,
`ErrorMessage`, `src/lib/api/**`; see [overview.md](overview.md)).

## Accounts (`src/features/accounts/`)

- `accountForm.ts` — pure, unit-tested helpers around the add/edit form:
  - `AccountFormState` — editable state (numeric fields kept as text while being edited).
  - `emptyAccountFormState(providerType)` / `accountToFormState(account)` — build the initial
    state for create/edit (the password field always starts empty on edit).
  - `existingTenant(account)` — the Microsoft tenant an existing account already has (the form
    never lets the user edit it; a new account defaults to `"organizations"`).
  - `validateAccountForm(state, isCreate)` — field-level validation (name, email contains `@`,
    IMAP host/port/username, password required only on create, folders non-empty, sync days
    1–365). `hasErrors(errors)` checks the result.
  - `buildAccountInput(state, tenant?)` — builds the exact `AccountInput` payload. The IMAP
    `password` key is only present when the user typed one, so editing without touching the
    password field leaves the stored password unchanged.
- `AccountFormModal.tsx` — the add/edit modal. The IMAP/Microsoft 365 switch is only enabled when
  creating (`account === null`); editing keeps the existing provider. Switching provider keeps the
  name, email, default university and first-sync days (only provider-specific fields reset).
  "Test connection" only shows when editing an existing account (a new account must be saved
  first). Saving an IMAP account closes the modal; saving a Microsoft account keeps it open
  and shows a "Sign in with Microsoft" button (`connectMicrosoftAccount`), with a "Waiting for
  sign-in in your browser…" message while that call is pending and "Signed in." afterwards.
  Editing the email address of a Microsoft account discards its stored sign-in on the backend,
  so the account list shows "Sign-in required" and the account has to be signed in again (the
  tenant is never editable). A second
  Save after a Microsoft account was created updates that account (`account ?? savedAccount`)
  instead of creating a duplicate. The password input uses `autoComplete="new-password"`; the
  Microsoft form carries the note "Your sign-in is saved in Windows Credential Manager, never in the
  database." The form state is initialized once per mount: `AccountsPage` passes
  `key={`${modalAccount?.id ?? 'new'}-${modalOpenCount}`}`, where `modalOpenCount` is incremented on
  every open, so each open (a different account, "Add", or the same item after a cancel) starts
  with fresh state and unsaved edits are discarded.
- `AccountsPage.tsx` — the list screen. Each row shows the provider, email, default university
  name (resolved from `getCatalog`, which lists the configured LMS universities), folders and a last-sync line ("last sync `<time>` OK", the
  sync error in red, or "Sign-in required" in red for a Microsoft account with no stored refresh
  token; the sync time uses `formatDateTime`). Row actions: Test (shows the
  `ConnectionTestDto` result inline via the shared `formatConnectionTestMessage`), Edit,
  Reconnect/Sign in (Microsoft only — label depends on `credentialsConfigured`), the `Enabled`
  toggle (`updateAccount` with only `enabled` flipped, everything else unchanged, no password sent;
  disabled while its own mutation is pending), and delete (opens `ConfirmDialog` with the exact
  copy "Deletes this account and all its results. Disable it instead to keep its history."; a
  failed delete shows its error inside the dialog). Mutations invalidate `queryKeys.accounts` and the
  dashboard query (`['dashboard']`, a prefix match covering every `sort` variant) so the sidebar's
  unseen badge and the Announcements attention banners stay in sync.

## Settings (`src/features/settings/`)

- `settingsForm.ts` — `settingsToFormState(settings)` / `buildSettingsInput(state)` convert between
  `SettingsDto`/`SettingsInput` and the editable form state; `parseBlockedSenders(raw)` splits the
  one-per-line textarea into a trimmed, non-empty array.
- `JevKeyModal.tsx` — the "Replace key" modal: a password input that only ever gets sent to
  `setJevApiKey`; the key is never read back or rendered anywhere, before or after saving.
- `SettingsPage.tsx` — provider cards (jev — "Sends email text to TypeSafe."; Ollama — "Stays on
  this PC."), exposed as a `role="radiogroup"` of `role="radio"` buttons with `aria-checked`; jev
  key status (`SettingsDto.jevApiKeyConfigured`, "Configured"/"Not configured", read-only, read
  straight from the query) with Replace/Remove key actions (a failed remove shows its error next to
  the button), jev model, Ollama URL and model, "Test provider" (shows the `ConnectionTestDto`
  message inline via `formatConnectionTestMessage`), the Microsoft Entra client ID, and the blocked
  senders/domains textarea. A single Save button submits the whole form via `updateSettings`. The
  form is initialized from `getSettings` only on the first load (state set during render while the
  form is still `null`) and re-synced from the response of a successful Save. It is deliberately
  not rebuilt on later refetches (e.g. after replacing/removing the jev key), so unsaved edits are
  never lost.

## Testing

Every component test mocks `../../lib/api/client` with `vi.mock(..., async (importOriginal) => ...)`,
keeping the real `toCommandError` but replacing the `api` methods the component under test calls.
Because mutations resolve asynchronously (TanStack Query dispatches `mutationFn` outside the
synchronous event-handler call stack), assertions on a mutation's side effect use
`await waitFor(() => expect(...))` rather than asserting immediately after `fireEvent.click`.

Covered cases (see the `*.test.ts(x)` files next to each module):
- IMAP add form validation (every required field) and the exact `addAccount` payload on success.
- Microsoft add payload (`tenant: 'organizations'`, `folders: ['inbox']`).
- The enable toggle calling `updateAccount` with `enabled: false` and the rest of the account
  unchanged.
- Delete requiring confirmation (the destructive call only fires after confirming).
- The inline "Test" result rendering a failed `ConnectionTestDto.message`.
- Settings save sending the parsed `blockedSenders` array.
- Replacing the jev key: `setJevApiKey` is called with the typed value, and that value is never
  found anywhere in the rendered output afterwards, and unsaved edits to other fields survive the
  settings refetch that follows.
- Cancelling an edit modal and reopening the same item shows the original values again
  (`AccountsPage.test.tsx`).
- Edit mode of the account modal: fields are prefilled and Save sends the edited payload
  (`updateAccount(id, …)`).
- A second Save after creating a Microsoft account calls `updateAccount`, not `addAccount`.
- A failed account delete renders its error inside the confirmation dialog.
- "Test provider" rendering the error message from a failed `ConnectionTestDto`.

## Known limitations

- The Microsoft sign-in UI only calls `connectMicrosoftAccount` and shows a pending/error state.
  The OAuth loopback flow itself runs in the backend: see
  [../infra/microsoft-graph.md](../infra/microsoft-graph.md) for the flow and
  [../app/tauri-commands.md](../app/tauri-commands.md) for the command (it waits up to 5 minutes for
  the browser redirect).
