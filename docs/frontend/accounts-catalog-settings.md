# Accounts, Catalog and Settings screens

Covers `src/features/accounts/**`, `src/features/catalog/**`, `src/features/settings/**` and the
shared form primitives in `src/components/forms/**`. These screens configure the app (email
accounts, the university/course catalog and global settings); they don't fetch results or trigger
a "Check now" run (see `docs/frontend/dashboard-results-runs.md` for that, once written).

## Shared form primitives (`src/components/forms/`)

- `Field` — label + control + optional hint/validation error, associated via `htmlFor`/`id`.
- `TextInput` / `Textarea` / `Select` — thin styled wrappers over the native elements, forwarding
  every native prop (`Select` takes an `options: { value, label }[]` list instead of children).
- `Toggle` — an accessible on/off switch (`role="switch"`, `aria-checked`) used for the account
  "Enabled" control.
- `ConfirmDialog` — a `Modal` wrapper for destructive-action confirmations (delete an account, a
  university or a course).
- `icons.tsx` — small inline SVG icons (`TrashIcon`, `PencilIcon`) used instead of emoji.

These are the only building blocks this task owns outside `src/features/**`; everything else
(`Layout`, `Modal`, `ErrorMessage`, `src/lib/api/**`) belongs to other tasks and is used as-is.

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
  creating (`account === null`); editing keeps the existing provider. "Test connection" only shows
  when editing an existing account (a new account must be saved first, per the design). Saving an
  IMAP account closes the modal; saving a Microsoft account keeps it open and shows a "Sign in with
  Microsoft" button (`connectMicrosoftAccount`), with a "Waiting for sign-in in your browser…"
  message while that call is pending.
- `AccountsPage.tsx` — the list screen. Each row shows the provider, email, default university
  name (resolved from `getCatalog`), folders and a last-sync line ("last sync `<time>` OK", the
  sync error in red, or "Sign-in required" in red for a Microsoft account with no stored refresh
  token). Row actions: Test (shows the `ConnectionTestDto` result inline), Edit, Reconnect/Sign in
  (Microsoft only — label depends on `credentialsConfigured`), the `Enabled` toggle
  (`updateAccount` with only `enabled` flipped, everything else unchanged, no password sent), and
  delete (opens `ConfirmDialog` with the exact copy "Deletes this account and all its results.
  Disable it instead to keep its history."). Mutations invalidate `queryKeys.accounts` and the
  dashboard query (`['dashboard']`, a prefix match covering every `sort` variant) so the sidebar's
  unseen badge and dashboard attention banner stay in sync.

## Catalog (`src/features/catalog/`)

- `catalogForm.ts` — `parseAliases(raw)`: splits a comma-separated alias input into a trimmed,
  non-empty array (used for both add and edit).
- `UniversityFormModal.tsx` / `CourseFormModal.tsx` — add/edit modals for a university (name only)
  and a course (name + aliases, scoped to a `universityId`). Both invalidate `queryKeys.catalog` on
  success.
- `CatalogPage.tsx` — one card per university (name, "Edit" / "Add course" actions, a delete
  icon) with its courses listed underneath ("Name · aliases: A, B", edit/delete icons per course).
  Deleting either shows a `ConfirmDialog` first.

## Settings (`src/features/settings/`)

- `settingsForm.ts` — `settingsToFormState(settings)` / `buildSettingsInput(state)` convert between
  `SettingsDto`/`SettingsInput` and the editable form state; `parseBlockedSenders(raw)` splits the
  one-per-line textarea into a trimmed, non-empty array.
- `JevKeyModal.tsx` — the "Replace key" modal: a password input that only ever gets sent to
  `setJevApiKey`; the key is never read back or rendered anywhere, before or after saving.
- `SettingsPage.tsx` — provider cards (jev — "Sends email text to TypeSafe."; Ollama — "Stays on
  this PC."), jev key status (`SettingsDto.jevApiKeyConfigured`, "Configured"/"Not configured",
  read-only) with Replace/Remove key actions, jev model, Ollama URL and model, "Test provider"
  (shows the `ConnectionTestDto` message inline), the Microsoft Entra client ID, and the blocked
  senders/domains textarea. A single Save button submits the whole form via `updateSettings`. The
  form is (re)initialized from `getSettings` by comparing the latest query data against the last
  value used to initialize it and updating state during render — the documented alternative to an
  effect for "adjust state when a prop/query result changes" — instead of `useEffect`, to avoid an
  extra render pass.

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
- `parseAliases` and a course add call receiving the parsed alias array.
- Settings save sending the parsed `blockedSenders` array.
- Replacing the jev key: `setJevApiKey` is called with the typed value, and that value is never
  found anywhere in the rendered output afterwards.
- "Test provider" rendering the error message from a failed `ConnectionTestDto`.

## Known limitations

- Catalog delete confirmation copy ("Deletes this university and all its courses." / "Deletes this
  course.") is not specified verbatim by the design spec (only the account deletion copy is); it
  follows the same pattern.
- The Microsoft sign-in flow only covers the UI side (calling `connectMicrosoftAccount` and showing
  a pending/error state); the actual OAuth loopback flow is implemented in `src-tauri` (a later
  task).
