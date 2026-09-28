# Dashboard, Results and Runs

Three screens built on the API client from `src/lib/api/` (see `docs/frontend/overview.md`):
the Dashboard (`/`), Results (`/results`) and Runs (`/runs`) pages, plus the shared result
components they (and later, other screens) reuse.

## Shared components (`src/components/results/`)

These are presentational building blocks with no data fetching of their own — pages pass them
data and callbacks.

- **`labels.ts`** — human labels for the DTO enums: `CATEGORY_LABELS`, `DATE_KIND_LABELS`,
  `PRIORITY_LABELS`, `RESULT_SORT_LABELS`. The model/provider and confidence are never shown
  anywhere in the UI, per the design spec.
- **`format.ts`** — date/time formatting, all via `Intl.DateTimeFormat('en-GB')`:
  `formatDate`, `formatDateTime`, `formatTime`, `formatResultDate(date, time)` (e.g.
  `"27 Oct 10:00"`, used for date chips), `formatRelativeDay` (`"today"` / `"yesterday"` / a
  short date), `formatDuration(startedAt, finishedAt)` (`null` while a run is still running).
- **`PriorityBadge`** — colored pill for `Priority` (urgent red, high amber, medium blue, low
  gray).
- **`CategoryLabel`** — neutral pill for `Category`.
- **`ResultDates`** — one chip per `ResultDateDto`, e.g. `"Exam: 27 Oct 10:00"`.
- **`ResultActions`** — "Open email" (via `@tauri-apps/plugin-opener`'s `openUrl`, when the
  result has a `sourceUrl`) or "Copy Message-ID" (via `navigator.clipboard.writeText`, when it
  has a `messageIdHeader`); renders nothing when neither is set. Shared by `ResultCard` and the
  Results row detail so the two screens never diverge on this behavior.
- **`ResultCard`** — the dashboard's result card: subject, priority badge, meta line
  (`sender · course (university) · account · received <date>`), category/date/flag chips, and
  the actions + "Seen" button. Takes `onSetSeen(id, seen)`; the caller owns the mutation.
- **`SortSelect`** — the `ResultSort` dropdown ("Nearest upcoming date", "Priority", "Received
  date"), reused by the dashboard header and the Results filters modal.

## Dashboard (`src/features/dashboard/`)

- **`DashboardPage.tsx`** — `getDashboard(sort)` (sort state defaults to `'upcoming'`); header
  with the last-check line (`"Last check today 09:14 · partial: 1 of 2 accounts failed"`, built
  from `lastRun.status`/`errorSummary`) and the "Check now" button; attention banners from
  `dashboard.attention`; five metric tiles (New, Tasks, Exams, Events, Changes and notices) from
  `dashboard.counts`; the "New results" list with the sort selector and "Mark all as seen"
  (`markAllSeen`, invalidates the dashboard/results queries); one `ResultCard` per
  `dashboard.newResults` entry, wired to `setResultSeen(id, true)`.
- **`useCheckRun.ts`** — the "Check now" state machine. Takes the dashboard's
  `runningRunId` (a check already running when the page loaded) so the running state survives a
  refresh; `triggerCheck()` calls `checkNow()` and starts tracking the returned run id either
  way. While a run id is tracked, it subscribes to `onRunProgress`/`onRunFinished` (unlistening
  on cleanup) and ignores events for other run ids. `onRunFinished` clears the running state and
  invalidates the dashboard/results/runs queries. `progressLabel` renders as e.g.
  `"UV IMAP: analyzing 12/40"` (counts are only shown for the `analyzing` stage); while running,
  the "Check now" button is replaced by this label per the design spec.

## Results (`src/features/results/`)

- **`ResultsPage.tsx`** — a search box (debounced 300 ms, trimmed before querying) and a
  "Filters and sort" button showing a badge with the number of active (non-sort) filters. Results
  are fetched with `getResults(query)`, paged 25 per page with Previous/Next (`offset`
  advances/retreats by the page size; `total` from the response drives the page count and button
  disabled state). Clicking a row toggles its expansion (only one row expanded at a time).
- **`FiltersModal.tsx`** — wraps the shared `Modal` with state (seen/new), category, account
  (from `getAccounts`), course (from `getCatalog`, flattened with the university name in the
  label) and sort. Edits are local (`draftFilters`/`draftSort`) until "Apply" commits them to the
  page and closes the modal; "Reset" clears the draft (state/category/account/course, sort back
  to `'upcoming'`) without closing, so the person can still cancel out. Also exports
  `countActiveFilters` for the badge.
- **`ResultRow.tsx`** — the table row (unseen dot, subject + sender/course/received date,
  category, next date, priority) and its expanded detail (sender with address, account, received
  and detected dates, all date chips, flags, `ResultActions`, and "Mark as seen"/"Mark as new"
  via `setResultSeen(id, !seen)`). "Next date" is the nearest date at or after today, or the
  earliest date when all are in the past, or `"—"` when the result has none.

## Runs (`src/features/runs/`)

- **`RunsPage.tsx`** — `getRunHistory(50)`, newest first (as returned by the backend); one
  `RunRow` per run, at most one expanded at a time; an empty state when there is no history yet.
- **`RunRow.tsx`** — status pill (Running/Success/Partial/Failed), start time
  (`formatDateTime`), duration (`formatDuration`, omitted while the run has no `finishedAt`),
  trigger ("Manual" — the only value `RunTrigger` currently has), and the counters line
  (`found · filtered · analyzed · failed · results`). Expanding shows the run's `errorSummary`
  (when partial/failed) and one block per account with its status, and for a failed account its
  error kind (human label, e.g. "Connection error") and message.

## Conventions followed

- No direct `invoke` calls; everything goes through `api` (`src/lib/api/client.ts`) and
  `queryKeys` (`src/lib/api/queryKeys.ts`).
- Mutations invalidate query key **prefixes** (`['dashboard']`, `['results']`, `['runs']`)
  rather than exact keys, since the dashboard/results queries are parameterized by sort/query and
  React Query's default `invalidateQueries` matching is prefix-based.
- The model/provider and confidence fields from `AIAnalysis`/`StoredResult` are never rendered.
- All UI copy is English, sentence case, no emoji.

## Testing

Every page/component is tested with Vitest + Testing Library. The API client module is mocked
per-file with `vi.mock('../../lib/api/client', ...)` (functions built with `vi.hoisted` so they
can be reset in `beforeEach`); `src/lib/api/events.ts` and `@tauri-apps/plugin-opener` are mocked
the same way wherever a test needs to simulate `run:progress`/`run:finished` or "Open email"/
"Copy Message-ID".

Run with `npm run test`. Other gates: `npm run typecheck`, `npm run lint`, `npm run build`.

## Known limitations

- `ResultsPage` fetches the full account and catalog lists on every mount to populate the filter
  dropdowns (via React Query's cache, so this is cheap after the first load); there is no
  dedicated "filters options" endpoint.
- The "Check now" progress label shows only the most recent `run:progress` event, not a running
  log — matching the single-line progress example in the design spec.
