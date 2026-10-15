# Announcements (email), Runs and shared result components

Email analysis is the secondary section of the app since Phase 2. Two screens remain, built on
the API client from `src/lib/api/` (see [overview.md](overview.md)): **Announcements**
(`/announcements`, the analyzed emails with the "Check now" header) and **Email runs** (`/runs`),
plus the shared result components they reuse. The main Dashboard (`/`) is now the LMS tracker,
documented in [tracker-universities.md](tracker-universities.md). The old email dashboard and the
`/results` route no longer exist: their content moved into Announcements.

## Shared components (`src/components/results/`)

These are presentational building blocks with no data fetching of their own — pages pass them
data and callbacks.

- **`labels.ts`** — human labels for the DTO enums: `CATEGORY_LABELS`, `DATE_KIND_LABELS`,
  `PRIORITY_LABELS`, `RESULT_SORT_LABELS`. The model/provider and confidence are never shown
  anywhere in the UI.
- **`format.ts`** — date/time formatting, all via `Intl.DateTimeFormat('en-GB')`:
  `formatDate`, `formatDateTime`, `formatTime`, `formatResultDate(date, time)` (e.g.
  `"27 Oct 10:00"`, used for date chips), `formatRelativeDay` (`"today"` / `"yesterday"` / a
  short date), `formatDuration(startedAt, finishedAt)` (`null` while a run is still running),
  `todayLocalIso()` (today's local calendar date as `YYYY-MM-DD`, never the UTC date).
- **`useSetResultSeen()`** — the `setResultSeen` mutation used by the Announcements table rows.
  Invalidates the `['dashboard']`/`['results']` prefixes and exposes `setSeen(id, seen)`,
  `isPending(id)`, and `error`/`erroredId` so a failure renders next to the row that triggered it.
- **`ResultFlags`** — the "Action required" / "Possible change" chips, used by `ResultRow`.
- **`PriorityBadge`** — colored pill for `Priority` (urgent red, high amber, medium blue, low
  gray).
- **`CategoryLabel`** — neutral pill for `Category`.
- **`ResultDates`** — one chip per `ResultDateDto`, e.g. `"Exam: 27 Oct 10:00"`.
- **`ResultActions`** — "Open email" (via `@tauri-apps/plugin-opener`'s `openUrl`, only when the
  result's `sourceUrl` parses and its protocol is `https:`) or "Copy Message-ID" (via
  `navigator.clipboard.writeText`, when it has a `messageIdHeader`); renders nothing when neither
  applies. Rejections are caught and shown as brief inline feedback ("Copied", "Couldn't open the
  email", "Couldn't copy"). Used in the expanded row detail.
- **`SortSelect`** — the `ResultSort` dropdown ("Nearest upcoming date", "Priority", "Received
  date"), used by the Announcements filters modal.

## Announcements (`src/features/announcements/`)

The route is `/announcements`. The sidebar link is disabled until at least one course is tracked
(see "Navigation" in [tracker-universities.md](tracker-universities.md)); the
backend also refuses "Check now" with "Track at least one course before checking email", because
the analyzer uses the tracked courses as its course catalog.

- **`AnnouncementsHeader.tsx`** — `getDashboard('upcoming')` feeds the header: the title, the
  last-check line (`"Last check today 09:14 · partial: 1 of 2 accounts failed"`, built from
  `lastRun.status`/`errorSummary`, or "No checks yet."), the "Check now" button (disabled while
  `triggerCheck` is pending; replaced by the progress label while a run is running; its error is
  shown below it), attention banners from `dashboard.attention`, five metric tiles (New, Tasks,
  Exams, Events, Changes and notices) from `dashboard.counts`, and "Mark all as seen"
  (`markAllSeen`, disabled while pending, error shown next to it, invalidates the dashboard and
  results queries).
- **`AnnouncementsPage.tsx`** — below the header, the former Results screen: a search box
  (debounced 300 ms, trimmed before querying) and a "Filters and sort" button showing a badge with
  the number of active (non-sort) filters. Results are fetched with `getResults(query)`, paged 25
  per page with Previous/Next (`offset` advances/retreats by the page size; `total` from the
  response drives the page count and button disabled state). The current page is clamped to the
  last page when `total` shrinks, and the query keeps its previous data while a new page or
  filter loads. The subject cell of each row is a `<button aria-expanded>` that toggles the
  row's expansion (only one row expanded at a time).

## Check-run state (`src/features/check-run/`)

The "Check now" state machine is app-level, not page-level: `CheckRunProvider` is mounted in
`src/main.tsx` and `useCheckRun()` returns `{ running, progress, error, triggerCheck }`. It
subscribes to `onRunProgress`/`onRunFinished` once on mount, so a fast `run:finished` (even one
arriving before `checkNow()` resolves) is never missed, and `run:finished` invalidates the
dashboard/results/runs prefixes on any route. `progress` renders as e.g.
`"UV IMAP: Analyzing 12/40"` (stage labels are human-readable; counts only for the `analyzing`
stage) and is only shown while it belongs to the running run. A run reported by the dashboard query
as `runningRunId` is adopted, and the running state is cleared when the dashboard shows
`runningRunId === null` and `lastRun.id >=` the tracked id. See [overview.md](overview.md).

## Result rows and filters (`src/features/results/`)

- **`FiltersModal.tsx`** — wraps the shared `Modal` with state (seen/new), category, account
  (from `getAccounts`), course (from `getCatalog`: the tracked courses, flattened with the
  university name in the label) and sort. Edits are local (`draftFilters`/`draftSort`) until
  "Apply" commits them to the Announcements page and closes the modal; "Reset" clears the draft
  (state/category/account/course, sort back to `'upcoming'`) without closing, so the person can
  still cancel out. Also exports
  `countActiveFilters` for the badge.
- **`ResultRow.tsx`** — the table row (unseen dot with an `sr-only` "Unseen" label, subject +
  sender/course/received date, category, next date, priority) and its expanded detail (sender with
  address, account, received and detected dates, all date chips, `ResultFlags`, `ResultActions`,
  and "Mark as seen"/"Mark as new" via `setResultSeen(id, !seen)`, disabled while pending with its
  error shown beside it). "Next date" is the nearest date at or after the local today
  (`todayLocalIso()`), or the earliest date when all are in the past, or `"—"` when the result has
  none.

## Runs (`src/features/runs/`)

- **`RunsPage.tsx`** — `getRunHistory(50)`, newest first (as returned by the backend); one
  `RunRow` per run, at most one expanded at a time; an empty state when there is no history yet.
- **`RunRow.tsx`** — status pill (Running/Success/Partial/Failed), start time
  (`formatDateTime`), duration (`formatDuration`, omitted while the run has no `finishedAt`),
  trigger ("Manual" — the only value `RunTrigger` currently has), and the counters line
  (`found · filtered · analyzed · failed · results`); the header is a `<button>` containing only
  inline elements (spans). Expanding shows the run's `errorSummary`
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
"Copy Message-ID". Components that use `useCheckRun()` (the Announcements header) must be rendered inside a
`CheckRunProvider` in tests.

Run with `npm run test`. Other gates: `npm run typecheck`, `npm run lint`, `npm run build`.

## Known limitations

- `AnnouncementsPage` fetches the full account and catalog lists on every mount to populate the filter
  dropdowns (via React Query's cache, so this is cheap after the first load); there is no
  dedicated "filters options" endpoint.
- The "Check now" progress label shows only the most recent `run:progress` event, not a running
  log.
- The check-run state is adopted from the dashboard query cache, so a check already running when
  the app starts is only noticed once some `['dashboard']` query has loaded (the sidebar's does).
