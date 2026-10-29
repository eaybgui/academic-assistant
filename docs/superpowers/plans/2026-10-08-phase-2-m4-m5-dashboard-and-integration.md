# Phase 2 · M4+M5 — Dashboard and integration: Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give the user the LMS tracker they asked for: course selection in the add-university wizard, a dashboard of deliverables and course events (All + one tab per university, grouped by time or status, badges, done/hide/notes, details), sync from the UI — and move email into a gated "Announcements" section, then bring the documentation up to date.

**Architecture:** The backend commands and events already exist (M2+M3). The frontend gets a typed API layer for them, an app-level `SyncProvider` (same pattern as `CheckRunProvider`), a pure grouping module, and React components on top. One backend change gates "Check now" until a course is tracked. Docs are rewritten last.

**Tech Stack:** React 19 + TypeScript (strict) + React Query 5 + react-router 7 (HashRouter) + Tailwind v4 + Vitest/Testing Library; Tauri 2 (`@tauri-apps/api` events, `@tauri-apps/plugin-opener`); Rust for Task 7 only.

**Spec:** `docs/superpowers/specs/2026-10-04-phase-2-lms-tracker-design.md` — §10, §11, §14 (M4, M5), **§18**, plus the amendments in `docs/superpowers/plans/2026-10-05-phase-2-m2-m3-items-and-sync.md` and in "Spec amendments made by this plan" below.

## Global Constraints

- Follow `AGENTS.md`: only your task, no unrelated refactors, **no new dependencies**, report deviations.
- **The GitHub repository is PUBLIC (AGENTS.md §11.2).** Tests and docs use invented data only ("Ana Pérez", "Universidad Demo", course "Álgebra Lineal", ids like `_10_1`). Never copy anything from the user's accounts or from `crates/infra/tests/fixtures/lms-local/`.
- All frontend command calls go through `src/lib/api/` (client, types, query keys, events). Never call `invoke`/`listen` elsewhere.
- UI copy in English, sentence case. Dates via the existing `src/components/results/format.ts` helpers (`en-GB`). Reuse existing components (`Modal`, `ConfirmDialog`, `ErrorMessage`, `forms/*`) and the existing Tailwind visual style (neutral palette, `rounded`, `text-sm`).
- Every rejected mutation/query is shown inline with `toCommandError` + `ErrorMessage`.
- `localStorage` access wrapped in try/catch (as `Layout.tsx` does for the sidebar preference).
- Links to the platform open only `https://` URLs with `openUrl` from `@tauri-apps/plugin-opener`.
- Never log or `console.*` item titles, notes or grades.
- Gates before every commit: `npm run typecheck`, `npm run lint`, `npm test`; Rust tasks also `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`.
- Git: each task on `feature/p2-m45-t{N}-{slug}` from `phase-2-lms-tracker`; commit there; never merge or push. Commit trailer: `Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>`.

## Spec amendments made by this plan

1. **"Check now" gating lives in the command layer**, not in `CheckRunner::prepare`: `src-tauri/src/commands/runs.rs::start_check` returns `validation` "Track at least one course before checking email" when the catalog has no tracked course. (Putting it in `core` would force rewriting every pipeline test fixture's empty catalog.)
2. **Routes:** `/` = tracker dashboard; `/announcements` = former Results page plus the email header (Check now, last check, counters, attention); `/universities`, `/accounts` (label "Email accounts"), `/runs` (label "Email runs"), `/settings`. The `/results` route and the email `DashboardPage` are removed.
3. **After saving tracked courses** (wizard or "Edit courses") the app starts a sync of that university immediately.
4. **"Edit courses"** first refreshes the course list from the platform (`lms_fetch_courses`); if that fails (e.g. not signed in) it falls back to the stored list (`get_courses`) and shows the error above the list.

## Review Focus

1. **A sync is already running when the user presses Sync or saves courses** → the `conflict` error ("A sync is already running") is shown inline and the UI keeps showing the running sync; nothing breaks. Pinned in Task 2 (`startSync` rejection).
2. **Dates around midnight and week boundaries** (Sunday 23:30, Monday 00:10, an item due later today) land in the right group in the user's local time. Pinned in Task 3 (fixed-`now` tests).
3. **Long or accented titles and missing course short names** sort and render sensibly (locale-aware sort, short name falls back to the course name). Pinned in Task 3 (sort) and Task 5 (row rendering).
4. **No universities, or universities with zero tracked courses** → the dashboard shows the first-run card / an empty state with a way forward, not a blank page. Pinned in Task 6.
5. **The note editor**: saving an empty or whitespace note clears it; a note over 2000 characters shows the backend's validation message inline and keeps the text. Pinned in Task 5.

## Task order

`T1 → T2 → T3 → T4 → T5 → T6 → T7 → T8 → T9`, strictly sequential.

---

### Task 1: Frontend API layer for courses, sync and the tracker

**Files:**
- Modify: `src/lib/api/types.ts`, `src/lib/api/client.ts`, `src/lib/api/queryKeys.ts`, `src/lib/api/events.ts`
- Test: `src/lib/api/client.test.ts`

**Interfaces:**
- Consumes: backend commands/events from M2+M3 Task 10 (contract below, exact).
- Produces (used by every later task):

```ts
// types.ts (append)
export type ItemKind = 'assignment' | 'quiz' | 'test' | 'event' | 'other';
export type EffectiveStatus = 'pending' | 'overdue' | 'submitted' | 'graded' | 'past_event';
export type LmsSyncStage = 'courses' | 'items' | 'saving' | 'done' | 'failed' | 'login_required';

export interface LmsCourseDto {
  id: number; universityId: number; name: string; shortName: string | null;
  url: string | null; tracked: boolean; available: boolean; isNew: boolean;
}
export interface GradeDto { display: string; score: number | null; max: number | null }
export type ItemChangeDto = { kind: 'new' } | { kind: 'due_changed'; previousDueAt: string | null };
export interface LmsItemDto {
  id: number; universityId: number; universityName: string; courseId: number;
  courseName: string; courseShortName: string | null; externalId: string; kind: ItemKind;
  title: string; dueAt: string | null; url: string | null; status: EffectiveStatus;
  grade: GradeDto | null; change: ItemChangeDto | null; removedAt: string | null;
  done: boolean; hidden: boolean; note: string | null;
}
export interface TrackerQuery { universityId: number | null; includeHidden: boolean; includeRemoved: boolean }
export interface TrackerDto { items: LmsItemDto[]; universities: LmsUniversityDto[]; syncRunning: boolean }
export interface LmsSyncResultDto {
  universityId: number; universityName: string; outcome: SyncOutcome;
  newItems: number; dueChanged: number; removed: number; error: string | null;
}
export interface LmsSyncProgressEvent { universityId: number; universityName: string; stage: LmsSyncStage }
export interface LmsSyncFinishedEvent { results: LmsSyncResultDto[] }
```

```ts
// client.ts (add to `api`)
lmsFetchCourses: (id: number) => invoke<LmsCourseDto[]>('lms_fetch_courses', { id }),
getCourses: (universityId: number) => invoke<LmsCourseDto[]>('get_courses', { universityId }),
setTrackedCourses: (universityId: number, courseIds: number[]) =>
  invoke<LmsCourseDto[]>('set_tracked_courses', { universityId, courseIds }),
lmsSync: (universityId: number | null) => invoke<void>('lms_sync', { universityId }),
getTracker: (query: TrackerQuery) => invoke<TrackerDto>('get_tracker', { query }),
setItemDone: (id: number, value: boolean) => invoke<LmsItemDto>('set_item_done', { id, value }),
setItemHidden: (id: number, value: boolean) => invoke<LmsItemDto>('set_item_hidden', { id, value }),
setItemNote: (id: number, note: string | null) => invoke<LmsItemDto>('set_item_note', { id, note }),
dismissItemChange: (id: number) => invoke<LmsItemDto>('dismiss_item_change', { id }),
dismissAllChanges: (universityId: number | null) => invoke<number>('dismiss_all_changes', { universityId }),
```

```ts
// queryKeys.ts (add)
tracker: (query: TrackerQuery) => ['tracker', query] as const,
courses: (universityId: number) => ['courses', universityId] as const,
```

```ts
// events.ts (add)
export function onLmsSyncProgress(cb: (event: LmsSyncProgressEvent) => void): Promise<UnlistenFn> {
  return listen<LmsSyncProgressEvent>('lms-sync:progress', (e) => cb(e.payload));
}
export function onLmsSyncFinished(cb: (event: LmsSyncFinishedEvent) => void): Promise<UnlistenFn> {
  return listen<LmsSyncFinishedEvent>('lms-sync:finished', (e) => cb(e.payload));
}
```

- [ ] **Step 1: Failing tests** in `client.test.ts`, one per new method, following the existing `invoke` mock style, asserting the command name and argument object exactly, e.g.:

```ts
it('setTrackedCourses sends universityId and courseIds', async () => {
  invokeMock.mockResolvedValue([]);
  await api.setTrackedCourses(2, [5, 7]);
  expect(invokeMock).toHaveBeenCalledWith('set_tracked_courses', { universityId: 2, courseIds: [5, 7] });
});

it('getTracker wraps the query', async () => {
  invokeMock.mockResolvedValue({ items: [], universities: [], syncRunning: false });
  const query = { universityId: null, includeHidden: false, includeRemoved: false };
  await api.getTracker(query);
  expect(invokeMock).toHaveBeenCalledWith('get_tracker', { query });
});
```

(Use the existing mock variable name in that file.) Run `npm test -- src/lib/api` → FAIL.

- [ ] **Step 2: Implement** the blocks above. Run tests → PASS; `npm run typecheck`, `npm run lint`.
- [ ] **Step 3: Commit** — `feat(ui): add API client for courses, sync and tracker`.

---

### Task 2: App-level `SyncProvider`

**Files:**
- Create: `src/features/sync/SyncProvider.tsx`, `src/features/sync/useSync.ts`, `src/features/sync/SyncProvider.test.tsx`
- Modify: `src/main.tsx` (wrap inside `CheckRunProvider`: `<SyncProvider>` around `<HashRouter>`)

**Interfaces:**
- Consumes: Task 1 (`api.lmsSync`, `onLmsSyncProgress`, `onLmsSyncFinished`, types).
- Produces:

```ts
// useSync.ts
export interface SyncState {
  /** True from a successful `startSync` until `lms-sync:finished`. */
  running: boolean;
  /** Human label of the latest progress event while running, e.g. "UV: Fetching items". */
  progress: string | null;
  /** Results of the last finished sync (empty array after an interrupted sync); null before any. */
  lastResults: LmsSyncResultDto[] | null;
  /** Error of the last `startSync` call (e.g. conflict "A sync is already running"). */
  error: CommandError | null;
  startSync: (universityId: number | null) => Promise<void>;
  clearResults: () => void;
}
export const SyncContext = createContext<SyncState | null>(null);
export function useSync(): SyncState; // throws outside the provider, like useCheckRun
```

Stage labels: `courses` "Checking courses", `items` "Fetching items", `saving` "Saving", `done` "Done", `failed` "Failed", `login_required` "Sign-in required". Progress text: `` `${universityName}: ${label}` ``.

Behavior (mirror `CheckRunProvider.tsx` and its comments):
- Subscribe to both events once on mount (with the `cancelled` unlisten pattern).
- `startSync(id)`: clears `error`, calls `api.lmsSync(id)`; on success sets `running = true`; on rejection sets `error = toCommandError(e)` and leaves `running` unchanged.
- On `lms-sync:finished`: `running = false`, `progress = null`, `lastResults = event.results`, and invalidate query prefixes `['tracker']`, `['universities']`, `['courses']`.
- On `lms-sync:progress`: set `progress` (also if a sync was started elsewhere, set `running = true`).

- [ ] **Step 1: Failing tests** (mock `../../lib/api/client` and `../../lib/api/events` exactly like `CheckRunProvider.test.tsx`; capture the callbacks passed to the `on*` mocks):
  - `startSync` success → `running` true; then firing finished with one result → `running` false, `lastResults` equal to the payload, and `invalidateQueries` called for `['tracker']`, `['universities']`, `['courses']` (spy on the QueryClient).
  - a progress event while running shows "UV: Fetching items".
  - `startSync` rejected with `{ kind: 'conflict', message: 'A sync is already running' }` → `error.message` shown, `running` stays false.
  - a progress event without a prior `startSync` sets `running` true (sync started from another view).
  - `useSync` outside the provider throws.
- [ ] **Step 2: Implement**, wire into `main.tsx`. Run tests → PASS; gates.
- [ ] **Step 3: Commit** — `feat(ui): add app-level LMS sync provider`.

---

### Task 3: Pure tracker grouping

**Files:**
- Create: `src/features/tracker/grouping.ts`, `src/features/tracker/grouping.test.ts`

**Interfaces:**
- Consumes: `LmsItemDto`, `EffectiveStatus` (Task 1).
- Produces:

```ts
export type TimeGroupKey = 'overdue' | 'today' | 'this_week' | 'next_week' | 'later' | 'no_date';
export type StatusGroupKey = 'pending' | 'submitted' | 'graded' | 'past_events';
export interface ItemGroup<K extends string> { key: K; label: string; items: LmsItemDto[] }
export interface GroupedItems<K extends string> { groups: ItemGroup<K>[]; completed: LmsItemDto[] }

export function isCompleted(item: LmsItemDto): boolean;
export function compareItems(a: LmsItemDto, b: LmsItemDto): number;
export function groupByTime(items: LmsItemDto[], now: Date): GroupedItems<TimeGroupKey>;
export function groupByStatus(items: LmsItemDto[]): GroupedItems<StatusGroupKey>;
```

- [ ] **Step 1: Write the tests first** (fixed dates; build items with a helper `item(overrides)` returning a full `LmsItemDto` with invented values):

```ts
const now = new Date(2026, 9, 7, 10, 0); // Wednesday 7 Oct 2026, 10:00 local

it('completed items are submitted, graded or past events', () => {
  expect(isCompleted(item({ status: 'submitted' }))).toBe(true);
  expect(isCompleted(item({ status: 'graded' }))).toBe(true);
  expect(isCompleted(item({ status: 'past_event' }))).toBe(true);
  expect(isCompleted(item({ status: 'overdue' }))).toBe(false);
  expect(isCompleted(item({ status: 'pending' }))).toBe(false);
});

it('groups open items by local time, weeks starting on Monday', () => {
  const at = (d: Date) => d.toISOString();
  const items = [
    item({ id: 1, status: 'overdue', dueAt: at(new Date(2026, 9, 6, 23, 59)) }),
    item({ id: 2, status: 'pending', dueAt: at(new Date(2026, 9, 7, 23, 30)) }),   // today
    item({ id: 3, status: 'pending', dueAt: at(new Date(2026, 9, 11, 23, 30)) }),  // Sunday → this week
    item({ id: 4, status: 'pending', dueAt: at(new Date(2026, 9, 12, 0, 10)) }),   // Monday → next week
    item({ id: 5, status: 'pending', dueAt: at(new Date(2026, 9, 18, 23, 0)) }),   // next Sunday → next week
    item({ id: 6, status: 'pending', dueAt: at(new Date(2026, 9, 19, 9, 0)) }),    // later
    item({ id: 7, status: 'pending', dueAt: null }),                                // no date
    item({ id: 8, status: 'graded', dueAt: at(new Date(2026, 9, 1, 9, 0)) }),      // completed
  ];
  const { groups, completed } = groupByTime(items, now);
  const ids = Object.fromEntries(groups.map((g) => [g.key, g.items.map((i) => i.id)]));
  expect(ids).toEqual({ overdue: [1], today: [2], this_week: [3], next_week: [4, 5], later: [6], no_date: [7] });
  expect(completed.map((i) => i.id)).toEqual([8]);
  expect(groups.map((g) => g.label)).toEqual(['Overdue', 'Today', 'This week', 'Next week', 'Later', 'No date']);
});

it('omits empty groups', () => {
  const { groups } = groupByTime([item({ status: 'pending', dueAt: null })], now);
  expect(groups.map((g) => g.key)).toEqual(['no_date']);
});

it('an open item due earlier today that the backend still calls pending stays in Today', () => {
  const { groups } = groupByTime([item({ status: 'pending', dueAt: new Date(2026, 9, 7, 9, 0).toISOString() })], now);
  expect(groups[0].key).toBe('today');
});

it('groups by status with overdue first inside pending', () => {
  const items = [
    item({ id: 1, status: 'pending', dueAt: '2026-10-09T10:00:00Z' }),
    item({ id: 2, status: 'overdue', dueAt: '2026-10-05T10:00:00Z' }),
    item({ id: 3, status: 'submitted' }),
    item({ id: 4, status: 'graded' }),
    item({ id: 5, status: 'past_event' }),
  ];
  const { groups, completed } = groupByStatus(items);
  expect(groups.map((g) => [g.key, g.items.map((i) => i.id)])).toEqual([
    ['pending', [2, 1]], ['submitted', [3]], ['graded', [4]],
  ]);
  expect(completed.map((i) => i.id)).toEqual([5]); // past events collapsed like "Completed"
});

it('sorts by due date, undated last, then title with a locale-aware compare', () => {
  const a = item({ id: 1, title: 'Zeta', dueAt: null });
  const b = item({ id: 2, title: 'álgebra', dueAt: null });
  const c = item({ id: 3, title: 'Beta', dueAt: '2026-10-08T10:00:00Z' });
  expect([a, b, c].sort(compareItems).map((i) => i.id)).toEqual([3, 2, 1]);
});
```

Run `npm test -- src/features/tracker/grouping` → FAIL.

- [ ] **Step 2: Implement**:

```ts
const COLLATOR = new Intl.Collator(undefined, { sensitivity: 'base', numeric: true });
const TIME_LABELS: Record<TimeGroupKey, string> = {
  overdue: 'Overdue', today: 'Today', this_week: 'This week', next_week: 'Next week', later: 'Later', no_date: 'No date',
};
const TIME_ORDER: TimeGroupKey[] = ['overdue', 'today', 'this_week', 'next_week', 'later', 'no_date'];
const STATUS_LABELS: Record<StatusGroupKey, string> = {
  pending: 'Pending', submitted: 'Submitted', graded: 'Graded', past_events: 'Past events',
};

export function isCompleted(item: LmsItemDto): boolean {
  return item.status === 'submitted' || item.status === 'graded' || item.status === 'past_event';
}

export function compareItems(a: LmsItemDto, b: LmsItemDto): number {
  if (a.dueAt !== b.dueAt) {
    if (a.dueAt === null) return 1;
    if (b.dueAt === null) return -1;
    const diff = Date.parse(a.dueAt) - Date.parse(b.dueAt);
    if (diff !== 0) return diff;
  }
  return COLLATOR.compare(a.title, b.title) || a.id - b.id;
}

function startOfDay(d: Date): Date { return new Date(d.getFullYear(), d.getMonth(), d.getDate()); }
function addDays(d: Date, days: number): Date { return new Date(d.getFullYear(), d.getMonth(), d.getDate() + days); }

function timeKey(item: LmsItemDto, now: Date): TimeGroupKey {
  if (item.dueAt === null) return 'no_date';
  if (item.status === 'overdue') return 'overdue';
  const due = new Date(item.dueAt);
  const today = startOfDay(now);
  const tomorrow = addDays(today, 1);
  const daysToMonday = ((8 - today.getDay()) % 7) || 7; // getDay: Sunday 0 … Saturday 6
  const nextMonday = addDays(today, daysToMonday);
  const mondayAfter = addDays(nextMonday, 7);
  if (due < tomorrow) return 'today';
  if (due < nextMonday) return 'this_week';
  if (due < mondayAfter) return 'next_week';
  return 'later';
}

function collect<K extends string>(order: K[], labels: Record<K, string>, keyed: [K, LmsItemDto][]): ItemGroup<K>[] {
  return order
    .map((key) => ({ key, label: labels[key], items: keyed.filter(([k]) => k === key).map(([, i]) => i).sort(compareItems) }))
    .filter((group) => group.items.length > 0);
}

export function groupByTime(items: LmsItemDto[], now: Date): GroupedItems<TimeGroupKey> {
  const open = items.filter((i) => !isCompleted(i));
  const completed = items.filter(isCompleted).sort(compareItems);
  return { groups: collect(TIME_ORDER, TIME_LABELS, open.map((i) => [timeKey(i, now), i])), completed };
}

export function groupByStatus(items: LmsItemDto[]): GroupedItems<StatusGroupKey> {
  const pending = items.filter((i) => i.status === 'pending' || i.status === 'overdue');
  const overdueFirst = (a: LmsItemDto, b: LmsItemDto) =>
    Number(b.status === 'overdue') - Number(a.status === 'overdue') || compareItems(a, b);
  const groups: ItemGroup<StatusGroupKey>[] = [
    { key: 'pending' as const, label: STATUS_LABELS.pending, items: pending.sort(overdueFirst) },
    { key: 'submitted' as const, label: STATUS_LABELS.submitted, items: items.filter((i) => i.status === 'submitted').sort(compareItems) },
    { key: 'graded' as const, label: STATUS_LABELS.graded, items: items.filter((i) => i.status === 'graded').sort(compareItems) },
  ].filter((g) => g.items.length > 0);
  return { groups, completed: items.filter((i) => i.status === 'past_event').sort(compareItems) };
}
```

(`completedLabel` for the collapsed section is decided by the page: "Completed" for time grouping, "Past events" for status grouping.) Check the Sunday edge: with `today` = Sunday, `daysToMonday` = 1. Run tests → PASS; gates.
- [ ] **Step 3: Commit** — `feat(ui): add tracker grouping by time and status`.

---

### Task 4: Course selection (wizard step, edit courses, card actions)

**Files:**
- Create: `src/features/universities/CourseSelectionList.tsx`, `src/features/universities/EditCoursesModal.tsx`, tests `CourseSelectionList.test.tsx`, `EditCoursesModal.test.tsx`
- Modify: `src/features/universities/AddUniversityModal.tsx` (+ its test), `src/features/universities/UniversityCard.tsx`, `src/features/universities/UniversitiesPage.tsx` (+ tests), `src/features/universities/useInvalidateUniversities.ts` (also invalidate `['courses']` prefix)

**Interfaces:**
- Consumes: Task 1 (`api.lmsFetchCourses`, `api.getCourses`, `api.setTrackedCourses`, `LmsCourseDto`), Task 2 (`useSync().startSync`).
- Produces:

```ts
// CourseSelectionList.tsx
export interface CourseSelectionListProps {
  courses: LmsCourseDto[];
  selected: ReadonlySet<number>;
  onChange: (next: Set<number>) => void;
}
```

Requirements:
- **CourseSelectionList**: a `fieldset` with legend "Courses"; one checkbox per course labelled with the course name; the short name in muted text after it when present and different from the name; a "New" tag when `isNew`; "Not visible on the platform" muted text when `!available`. "Select all" / "Select none" text buttons. Sorted by name with `localeCompare`.
- **AddUniversityModal step 3**: after a successful sign-in (the existing "Connected as …" state) the footer button is "Choose courses" (instead of "Done"). It calls `api.lmsFetchCourses(id)` (pending text "Loading courses…"), then shows `CourseSelectionList` with **every course pre-selected**. Footer: "Save and sync" → `api.setTrackedCourses(id, [...selected])` → `startSync(id)` → `onClose()` → `navigate('/')`. Errors inline; the user can retry. Selecting zero courses is allowed (saves, syncs nothing).
- **EditCoursesModal** (`open`, `university`, `onClose`): on open, `api.lmsFetchCourses(university.id)`; if that rejects, show the error and load `api.getCourses(university.id)` instead. Pre-selected = `tracked`. "Save" → `setTrackedCourses` → `startSync(university.id)` → close. If `startSync` reports an error (e.g. a sync already running) the modal still closes after saving and the error is shown by the dashboard's sync state — do not block saving on it.
- **UniversityCard**: add "Sync {name}" (`aria-label`, text "Sync") when `loginState === 'ok'` → `startSync(id)`, disabled while `useSync().running`; add "Edit courses of {name}" (text "Edit courses") when `loginState === 'ok'`; when `courses.unreviewed > 0 && courses.total > courses.unreviewed`, show a link-style button "{n} new courses found" opening the same modal; show "Last synced …" (`formatRelativeDay` + `formatTime` of `lastSync.at`) and, when `lastSync.outcome === 'failed'`, the error text in red.
- **UniversitiesPage**: when navigated to with `location.state?.openAdd === true` (react-router `useLocation`), open the add wizard on mount once.

- [ ] **Step 1: Failing tests** (mock `api` and wrap renders in `QueryClientProvider` + `MemoryRouter`; mock `useSync` via `vi.mock('../sync/useSync', …)` returning `{ startSync: vi.fn().mockResolvedValue(undefined), running: false, … }`):
  - CourseSelectionList: renders names, the "New" tag and "Not visible on the platform"; "Select none" then ticking one calls `onChange` with that id only.
  - AddUniversityModal: after sign-in, "Choose courses" loads courses, all checkboxes checked; unticking one and "Save and sync" calls `setTrackedCourses(1, [ids except that one])`, then `startSync(1)`, then navigates to `/` (assert with a test route `<Route path="/" element={<p>tracker</p>} />`).
  - EditCoursesModal: `lmsFetchCourses` rejected → error text shown and `getCourses` used; ticks reflect `tracked`; Save calls `setTrackedCourses` and `startSync`.
  - UniversityCard: "Sync UV" calls `startSync(1)`; "2 new courses found" shown for `{ total: 5, tracked: 3, unreviewed: 2 }` and not for `{ total: 5, tracked: 0, unreviewed: 5 }`.
  - UniversitiesPage: rendered at `/universities` with `state: { openAdd: true }` shows the "Add university" dialog.
- [ ] **Step 2: Implement.** Run tests → PASS; gates.
- [ ] **Step 3: Commit** — `feat(ui): add course selection to the university wizard and cards`.

---

### Task 5: Item row, details panel and item mutations

**Files:**
- Create: `src/features/tracker/labels.ts`, `src/features/tracker/useItemMutations.ts`, `src/features/tracker/ItemRow.tsx`, `src/features/tracker/ItemDetailsModal.tsx`, tests `ItemRow.test.tsx`, `ItemDetailsModal.test.tsx`

**Interfaces:**
- Consumes: Task 1 (`LmsItemDto`, item `api` methods), `format.ts` helpers.
- Produces:

```ts
// labels.ts
export const KIND_LABELS: Record<ItemKind, string> = { assignment: 'Assignment', quiz: 'Quiz', test: 'Test', event: 'Event', other: 'Other' };
export const STATUS_LABELS: Record<EffectiveStatus, string> = { pending: 'Pending', overdue: 'Overdue', submitted: 'Submitted', graded: 'Graded', past_event: 'Past' };
export function courseLabel(item: Pick<LmsItemDto, 'courseName' | 'courseShortName'>): string; // short name, else name
export function changeText(change: ItemChangeDto): string; // "New" | "Date changed: {old} → {new}" | "Date changed: no date → {new}"
// useItemMutations.ts — every mutation invalidates the ['tracker'] prefix on success
export function useItemMutations(): {
  setDone: UseMutationResult<LmsItemDto, unknown, { id: number; value: boolean }>;
  setHidden: UseMutationResult<LmsItemDto, unknown, { id: number; value: boolean }>;
  setNote: UseMutationResult<LmsItemDto, unknown, { id: number; note: string | null }>;
  dismissChange: UseMutationResult<LmsItemDto, unknown, number>;
};
// ItemRow.tsx
export interface ItemRowProps { item: LmsItemDto; showUniversity: boolean; onOpen: (item: LmsItemDto) => void }
// ItemDetailsModal.tsx
export interface ItemDetailsModalProps { item: LmsItemDto | null; onClose: () => void }
```

`changeText` for `due_changed` formats both dates with `formatShortDate`-style helpers from `format.ts` (use the existing short date + time functions; if a helper is missing, add it to `format.ts` with a test).

Requirements:
- **ItemRow** (an `li`): university chip (university name, small, only when `showUniversity`); title as a button (`onOpen`); muted "{courseLabel} · {KIND_LABELS[kind]}"; status pill (`STATUS_LABELS`, red for overdue, green for submitted/graded, neutral otherwise); grade display when present; due date/time (or "No date"); change badge (`changeText`, amber) with a dismiss button `aria-label="Dismiss change for {title}"`; a checkbox `aria-label="Mark {title} as done"` bound to `item.done` (disabled for `event` items); a note indicator (`aria-label="Has a note"`) when `note`; muted "Removed from platform" when `removedAt`. Mutation errors inline under the row.
- **ItemDetailsModal** (`Modal`, title = item title): university · course · kind; due date and time in full; status; grade; change text; "Removed from the platform on {date}" when removed; "Open in {platform name}" button only when `url` starts with `https://` (platform name from the item's university is not in the DTO — use the label "Open in platform"); note `Textarea` (`id="item-note"`, label "Note") with "Save note" → `setNote({ id, note: text.trim() === '' ? null : text })` (the backend trims and validates; show its error inline and keep the text); "Hide" / "Show again" toggle → `setHidden`; "Mark as done" / "Mark as not done" → `setDone` (hidden for events).

- [ ] **Step 1: Failing tests** (mock `api`, mock `@tauri-apps/plugin-opener` as `ResultActions.test.tsx` does):
  - ItemRow renders title, `courseShortName` (and the course name when the short name is null), the "Overdue" pill, the grade, and "Date changed: 5 Oct → 9 Oct" style text (assert with a regex on "Date changed"); clicking the done checkbox calls `setItemDone(id, true)`; the dismiss button calls `dismissItemChange(id)`; no checkbox for events.
  - ItemDetailsModal: "Open in platform" calls `openUrl` with the item url, and is absent for an `http://` url; saving "   " calls `setItemNote(id, null)`; a rejected `setItemNote` with `{ kind: 'validation', message: 'Notes can have at most 2000 characters' }` shows that message and keeps the textarea text; "Hide" calls `setItemHidden(id, true)`.
- [ ] **Step 2: Implement.** Run tests → PASS; gates.
- [ ] **Step 3: Commit** — `feat(ui): add tracker item row and details panel`.

---

### Task 6: Tracker dashboard page

**Files:**
- Create: `src/features/tracker/TrackerPage.tsx`, `src/features/tracker/SyncBanners.tsx`, `src/features/tracker/usePersistentState.ts`, tests `TrackerPage.test.tsx`, `usePersistentState.test.ts`
- Modify: `src/App.tsx` (route `/` → `TrackerPage`; keep the other routes for now — Task 8 rewires email routes)

**Interfaces:**
- Consumes: Tasks 1–5.
- Produces: `usePersistentState<T extends string>(key: string, fallback: T, allowed: readonly T[]): [T, (v: T) => void]` — reads/writes `localStorage` with try/catch; ignores stored values not in `allowed`.

Requirements:
- Data: `useQuery({ queryKey: queryKeys.tracker(query), queryFn: () => api.getTracker(query) })` with `query = { universityId: tab === 'all' ? null : Number(tab), includeHidden, includeRemoved }`. Universities come from `TrackerDto.universities`.
- **First run**: when the loaded `universities` list is empty, render only a card: heading "Track your deliverables", text "Add your university to see assignments, exams and course events in one place.", button "Add your first university" → `navigate('/universities', { state: { openAdd: true } })`.
- **No tracked courses** (universities exist, every `courses.tracked === 0`): empty state "No courses are tracked yet." with a link button "Choose courses" → `/universities`.
- **Top bar**: heading "Dashboard"; tab list (`role="tablist"`, buttons `role="tab"`, `aria-selected`) "All" + one per university name (persisted key `academic-assistant.tracker-tab`, values `'all'` or the id as string; falls back to `'all'` if the stored id no longer exists); "Group by" segmented control "Time" / "Status" (persisted key `academic-assistant.tracker-group`); checkboxes "Show hidden", "Show removed" (not persisted); "Sync" button → `startSync(tab === 'all' ? null : id)`, disabled and labelled with `progress` while `running`; "Last synced {relative}" from the most recent `lastSync.at` among the shown universities.
- **Groups**: time grouping → `groupByTime(items, new Date())`, collapsed `<details>` "Completed ({n})" at the end; status grouping → `groupByStatus(items)` with collapsed "Past events ({n})". Each group: `section` with an `h2` label and the count; `ItemRow` per item with `showUniversity = tab === 'all'`. No items at all → "Nothing to show. Press Sync to fetch your courses' items." Clicking a title opens `ItemDetailsModal`.
- **SyncBanners** (`role="alert"` per banner):
  - per shown university with `loginState === 'expired' || lastSync.outcome === 'login_required'`: "{name} needs you to sign in again" + button "Sign in" → `api.lmsSignIn(id)` then `startSync(id)`; errors inline.
  - after a sync (`useSync().lastResults` non-null): summary "Sync finished: {newItems} new, {dueChanged} date changed, {removed} removed" (sums over results), plus one line per failed result "{universityName}: {error}"; buttons "Dismiss all changes" → `api.dismissAllChanges(null)` (then invalidate `['tracker']`) and "Close" → `clearResults()`. An interrupted sync (`lastResults` = `[]`) shows "The sync was interrupted. Try again."
  - `useSync().error` shown inline next to the Sync button.

- [ ] **Step 1: Failing tests** (`TrackerPage.test.tsx`; mock `api`, `useSync`, and render with `MemoryRouter` + routes for `/` and `/universities`; clear `localStorage` in `beforeEach`):
  - empty universities → first-run card; the button navigates to `/universities` (assert the test route renders).
  - universities with no tracked courses → "No courses are tracked yet."
  - items render in the right groups (use dates relative to the real `new Date()`: one overdue, one in 30 days, one graded) and "Completed (1)" is present.
  - switching to the "UAH" tab calls `getTracker` with `universityId: 2` and persists `'2'`; re-rendering restores the UAH tab.
  - "Group by Status" shows "Pending" and "Graded" headings.
  - "Show hidden" calls `getTracker` with `includeHidden: true`.
  - "Sync" on the All tab calls `startSync(null)`; on a university tab `startSync(id)`; while `running` the button is disabled and shows the progress text.
  - a university with `loginState: 'expired'` shows "UV needs you to sign in again"; "Sign in" calls `lmsSignIn(1)` then `startSync(1)`.
  - `lastResults` with one success (2 new) and one failure shows "Sync finished: 2 new, 0 date changed, 0 removed" and the failure line; "Dismiss all changes" calls `dismissAllChanges(null)`.
  - `usePersistentState` ignores a stored value not in `allowed` and survives `localStorage` throwing.
- [ ] **Step 2: Implement.** Run tests → PASS; gates.
- [ ] **Step 3: Commit** — `feat(ui): add the tracker dashboard`.

---

### Task 7: Gate "Check now" until a course is tracked (backend)

**Files:**
- Modify: `src-tauri/src/commands/runs.rs` (`start_check`), tests in the same file

**Interfaces:**
- Consumes: `state.catalog.catalog()` (returns tracked, available courses).
- Produces: `check_now` → `CommandError { kind: validation, message: "Track at least one course before checking email" }` when `catalog.courses` is empty, checked **before** `runner.prepare` (so no run row is created).

- [ ] **Step 1: Failing tests** in `runs.rs` tests (harness): with no universities/courses, `start_check` returns that validation error and `state.runs.latest()` is `None`; after inserting a university, upserting one course and `set_tracked_courses`, `start_check` proceeds to the existing behavior (with an enabled fake account it returns a run id). Adjust any existing `check_now`/`start_check` test that now needs a tracked course to keep its original intent (add a small helper `track_one_course(&state)` in `test_support.rs`).
- [ ] **Step 2: Implement** (≤ 10 lines). Gates (Rust).
- [ ] **Step 3: Commit** — `feat(app): require a tracked course before checking email`.

---

### Task 8: Announcements section and navigation

**Files:**
- Create: `src/features/announcements/AnnouncementsPage.tsx`, `src/features/announcements/AnnouncementsHeader.tsx`, `src/features/announcements/AnnouncementsPage.test.tsx`
- Move/modify: content of `src/features/results/ResultsPage.tsx` into the announcements page (keep `FiltersModal`, `ResultRow`, `filterUtils` where they are, imported from `features/results/`); delete `src/features/results/ResultsPage.tsx` + its test (port the relevant test cases to `AnnouncementsPage.test.tsx`); delete `src/features/dashboard/` (email dashboard) after moving its header pieces (last check text, Check now + progress, attention banners, metric tiles, "Mark all as seen") into `AnnouncementsHeader.tsx`
- Modify: `src/App.tsx`, `src/components/Layout.tsx`, `src/components/Layout.test.tsx`

**Interfaces:**
- Consumes: existing email API (`getDashboard`, `getResults`, `markAllSeen`, `useCheckRun`), `api.getUniversities` (for gating).
- Produces: routes and nav per amendment 2.

Requirements:
- Nav items in order: Dashboard (`/`, `end: true`), Announcements (`/announcements`, unseen badge as before), Universities, Email accounts (`/accounts`), Email runs (`/runs`), Settings. `RESULTS_PATH` badge logic moves to `/announcements`.
- Announcements gating: query `queryKeys.universities` → `api.getUniversities` (`retry: false`); when loaded and the sum of `courses.tracked` is 0, render the Announcements item as a non-link `span` with `aria-disabled="true"` and `title="Add a university and track courses first"`, muted style. While loading or on error, render it enabled.
- `/announcements` page: `AnnouncementsHeader` (heading "Announcements", last check line, Check now/progress, attention banners, the five counters, "Mark all as seen") above the former Results list with search, filters, paging and details unchanged. A `check_now` validation error is shown inline as today.
- Remove `/results` and the email `DashboardPage`; nothing else may still import them.

- [ ] **Step 1: Failing tests**:
  - `Layout.test.tsx`: labels are `['Dashboard', 'Announcements', 'Universities', 'Email accounts', 'Email runs', 'Settings']`; Announcements is disabled (`aria-disabled`) when `getUniversities` resolves with zero tracked courses and a link when one course is tracked; the unseen badge shows on Announcements.
  - `AnnouncementsPage.test.tsx`: renders the header counters from a mocked `getDashboard`, "Check now" calls `checkNow` (via the `CheckRunProvider` mock pattern), and the results list from a mocked `getResults` (port 2–3 cases from the deleted Results/Dashboard tests).
- [ ] **Step 2: Implement.** Run `npm test`, typecheck, lint → PASS.
- [ ] **Step 3: Commit** — `feat(ui): move email into an Announcements section`.

---

### Task 9: Documentation and acceptance checklist

**Files:**
- Modify: `README.md`, `requirements.md`, `technical-specification.md`, `docs/README.md`, `docs/architecture.md`, `docs/infra/database.md`, `docs/infra/secure-storage.md`, `docs/app/tauri-commands.md`, `docs/frontend/overview.md`, `docs/frontend/dashboard-results-runs.md`, `docs/frontend/accounts-catalog-settings.md`, `docs/user-guide.md`, `docs/testing.md`, `docs/core/domain-and-ports.md`, `docs/development.md`
- Create: `docs/infra/lms.md` (Moodle/Blackboard clients, sign-in capture, sync), `docs/frontend/tracker-universities.md`
- Modify: `docs/superpowers/specs/2026-10-04-phase-2-lms-tracker-design.md` — append **§19 "As built"** listing every amendment from the M2+M3 plan, this plan, and the real-data findings (Blackboard `resource/x-bb-assessment` → Assignment, attendance columns skipped, 403/404 course gradebooks → no items, 105-day calendar chunks due to the server's 16-week local-time limit, Moodle `-1` = no grade, `NotApplicable` + past → PastEvent, events outside the sync window never removed, "Check now" gating in the command layer). No real values.
- Leave `docs/developer-guide.html` unchanged but add a line at its top in `docs/README.md`'s index: "developer-guide.html describes Phase 1 only".

**Interfaces:** none (docs only).

Requirements:
- Describe the product as Phase 2 ships it: universities + sign-in, course selection, sync and change badges, dashboard, Announcements (email, gated), privacy model (scoped tokens/cookies in Credential Manager; the Credential Manager target format is `{key}.academic-assistant`; nothing sent anywhere except the platforms and, for email analysis, the chosen AI provider).
- `docs/app/tauri-commands.md`: every current command with arguments and return types (from `src-tauri/src/lib.rs` and `src/lib/api/client.ts`), plus the `lms-sync:*` events.
- `docs/infra/database.md`: migrations 0001–0003 and the current tables (no manual catalog).
- `docs/testing.md`: keep existing sections; add "Phase 2 acceptance checklist" for the user: add both universities, choose courses, first sync, dashboard tabs and grouping, mark done / hide / note, details "Open in platform", a second sync shows no badges, edit courses then sync, sign-in banner after an expired session, Announcements disabled before any tracked course and enabled after, Check now works. Also the live-test commands (`lms_live`) and the capture tool usage with the reminder that captures stay in the git-ignored `lms-local/` folder.
- All examples invented (AGENTS.md §11.2). Run `npm run lint` only if docs touch code; otherwise no gates beyond `git diff --check`.
- [ ] **Step 1:** Write the docs. **Step 2:** `git diff --check`. **Step 3: Commit** — `docs: describe the LMS tracker (phase 2)`.

---

## After M4+M5

The planner merges, runs all gates, then the user runs the "Phase 2 acceptance checklist". When it passes, `phase-2-lms-tracker` is merged into `main` (the user's validation gate) and, only with the user's explicit OK, pushed — after a final `git log --all -S` scan for personal data.
