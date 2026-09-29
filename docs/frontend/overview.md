# Frontend overview

The frontend is a Vite + React + TypeScript (strict) single-page app that runs inside a
Tauri 2 window. It talks to the Rust backend exclusively through Tauri commands and events;
there is no other network or IPC layer.

## Structure

```
index.html                  Vite entry point
vite.config.ts               Vite + Vitest config (dev server on port 1420, Tailwind plugin)
eslint.config.js             Flat ESLint config (typescript-eslint, react-hooks, react-refresh)
vitest.setup.ts              Test environment setup (jest-dom matchers, <dialog> polyfill)
src/
  main.tsx                   App entry: QueryClientProvider + CheckRunProvider + HashRouter
  App.tsx                    Route table
  index.css                  Tailwind entry point (`@import "tailwindcss";`)
  components/
    Layout.tsx                Sidebar navigation + content outlet
    Modal.tsx                 Native <dialog>-based modal
    ErrorMessage.tsx           Inline CommandError renderer
    forms/                    Field, TextInput, Textarea, Select, Toggle, ConfirmDialog, icons
    results/                  Result card, chips, labels, formatting helpers (shared by three screens)
  features/
    check-run/                App-level "Check now" state (CheckRunProvider, useCheckRun)
    dashboard/DashboardPage.tsx
    results/ResultsPage.tsx
    runs/RunsPage.tsx
    accounts/AccountsPage.tsx
    catalog/CatalogPage.tsx
    settings/SettingsPage.tsx
  lib/api/
    types.ts                  TypeScript mirror of the DTOs in src-tauri/src/dto/
    client.ts                 `api` object: one method per Tauri command
    events.ts                 `onRunProgress` / `onRunFinished` subscriptions
    queryKeys.ts               React Query key factories
  lib/format.ts               `formatConnectionTestMessage` (shared by every "test connection" UI)
```

The shared shell (`Layout`, `Modal`, `ErrorMessage`) and `src/lib/api/` are stable; screens should
not need to change them. The screens are documented in
[dashboard-results-runs.md](dashboard-results-runs.md) and
[accounts-catalog-settings.md](accounts-catalog-settings.md).

## Modal (`src/components/Modal.tsx`)

The `<dialog>` element itself is always mounted, but its body (title bar, children, footer) is
rendered only while `open` is true. Callers that keep form state in the component that renders the
`Modal` therefore also need a fresh mount per edited item: the pages pass `key={item?.id ?? 'new'}`
to the form modals (`AccountFormModal`, `UniversityFormModal`, `CourseFormModal`) so their state is
re-initialized whenever a different item (or "add") is opened. On open, focus moves to the first
focusable element of the body (falling back to the dialog itself). The dialog closes on Escape, the
close button, or a backdrop click - the latter only when both `mousedown` and `mouseup` land on the
backdrop, so dragging out of a text field does not dismiss it.

## Check-run state (`src/features/check-run/`)

`CheckRunProvider` is mounted once in `main.tsx` (inside `QueryClientProvider`) and `useCheckRun()`
(in `useCheckRun.ts`) returns `{ running, progress, error, triggerCheck }`. It subscribes to
`run:progress` and `run:finished` once on mount (cleaned up on unmount), so a run that finishes
before `checkNow()` resolves is never missed, and every route reacts to a finished run. Finished run
ids are kept in a ref and a `checkNow()` result for an already-finished id is ignored. It also
watches the query cache for `['dashboard']` queries: it adopts a `runningRunId` reported by the
backend and clears the running state once the dashboard shows `runningRunId === null` and
`lastRun.id >= ` the tracked id. On `run:finished` it invalidates the `['dashboard']`, `['results']`
and `['runs']` query prefixes.

## API client (`src/lib/api/client.ts`)

`api` is a plain object with one method per backend command (see
[../app/tauri-commands.md](../app/tauri-commands.md) for the full command table). Every method wraps `@tauri-apps/api/core`'s `invoke` with the exact command
name and argument object the backend expects — e.g. `api.getDashboard(sort)` calls
`invoke('get_dashboard', { sort })`.

Command rejections are `CommandError`-shaped (`{ kind, message }`, see `types.ts`). Use
`toCommandError(e)` to normalize any rejection into a `CommandError`: values that already look
like one pass through unchanged; anything else (a thrown string, a plain `Error`, `null`, a
network failure outside of Tauri, etc.) becomes `{ kind: 'internal', message: 'Something went
wrong' }`. Always go through `toCommandError` before showing an error to the user or passing it
to `ErrorMessage` — never assume the shape of a caught value.

```ts
try {
  await api.addAccount(input);
} catch (e) {
  setError(toCommandError(e));
}
```

## Events (`src/lib/api/events.ts`)

Two Tauri events are emitted by the backend during a "Check now" run:

- `onRunProgress(cb)` subscribes to `run:progress` (`RunProgressEvent`: run/account ids and
  names, `stage`, `processed`/`total`).
- `onRunFinished(cb)` subscribes to `run:finished` (`RunFinishedEvent`: run id and final
  `RunStatus`).

Both return `Promise<UnlistenFn>`. Call the resolved unlisten function on cleanup (typically in a
`useEffect` teardown) to avoid leaking listeners across renders/navigations.

## Query keys (`src/lib/api/queryKeys.ts`)

`queryKeys` centralizes the React Query keys used across the app (`dashboard(sort)`,
`results(query)`, `runs(limit)`, `accounts`, `catalog`, `settings`). Always use these factories
instead of writing array literals inline, so a query and its invalidations can never drift apart
because of a typo.

## Conventions

- UI copy is English, sentence case (e.g. "Check now", not "Check Now" or "CHECK NOW").
- No component library; styling is Tailwind CSS v4 utility classes with a neutral light theme.
- Modals use the native `<dialog>` element (`components/Modal.tsx`), not a custom overlay.
- Routing is a hash router (`react-router`'s `HashRouter`), so routes work when the built app is
  served from bundled files without server-side routing.
- Errors from commands are shown inline, next to the control that triggered them, via
  `ErrorMessage`.
- Data fetching goes through React Query with the `api` client and `queryKeys`; no direct
  `invoke` calls outside `src/lib/api/`.

## Adding a screen

1. Add or extend a page component under `src/features/<area>/`.
2. If it reads backend state, add a query using `useQuery({ queryKey: queryKeys.<x>(...), queryFn: () => api.<method>(...) })`.
3. If it writes, use `useMutation`, invalidate the relevant `queryKeys` entry on success, and
   surface failures with `toCommandError` + `ErrorMessage`.
4. Reuse `Layout` (already wraps every route via `App.tsx`) and `Modal` for any dialog; do not
   fork their implementation.
5. Register the route in `src/App.tsx` if it's a new top-level page, and add a link in
   `src/components/Layout.tsx`'s `NAV_ITEMS` (that file is otherwise stable — only extend the nav
   list, don't change its behavior).

## Testing

Run with `npm run test` (Vitest + jsdom + Testing Library). Component tests render through
`@testing-library/react`; `vitest.setup.ts` registers jest-dom matchers and a minimal
`<dialog>` polyfill (`showModal`/`close`) since jsdom does not implement native dialog behavior.

To test a component that calls the API, mock the client module rather than `invoke` directly:

```ts
vi.mock('../lib/api/client', () => ({
  api: { getDashboard: vi.fn().mockResolvedValue(dashboardFixture) },
}));
```

For `src/lib/api/client.ts` itself, mock `@tauri-apps/api/core`'s `invoke` and assert the exact
command name and argument object for each method (see `client.test.ts`).

Other gates: `npm run typecheck` (`tsc -b --noEmit`), `npm run lint` (ESLint flat config,
ignoring `dist/`, `src-tauri/`, `target/`, `crates/`), and `npm run build` (type-checks then
produces `dist/`, which Tauri packages as `frontendDist`).

## Known limitations

- `@tauri-apps/plugin-opener` is used only by `ResultActions` ("Open email", `https:` URLs only).
