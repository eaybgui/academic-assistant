# Calendar and Personal Tasks Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add personal tasks (a title, one day, an optional note, done) that the user creates in a new Calendar section or from the dashboard. They appear next to platform items in the dashboard list and on a month calendar with coloured dots.

**Architecture:**
- **Storage:** personal tasks get their own table, repository port and commands. `lms_items` and the sync are not touched.
- **Transport:** `get_tracker` additionally returns every personal task, so the dashboard and the calendar each make one request.
- **Frontend:** both kinds are wrapped in one `TrackerEntry` union. Grouping, rows, details and the calendar work on entries, so no logic is duplicated per kind.

**Tech Stack:**
- Rust: `crates/core`, `crates/infra` (rusqlite) and `src-tauri` (Tauri 2).
- Frontend: React 19, TypeScript (strict), React Query 5, react-router 7, Tailwind v4, Vitest with Testing Library.

**Spec:** `docs/superpowers/specs/2026-11-03-calendar-and-personal-tasks-design.md`. It is the binding authority. Read §4 (anti-goals), §5 (user decisions), §6 (constraints) and §9 (calls made).

## Global Constraints

- **Rules:** follow `AGENTS.md`.
  - Do only your own task, with no unrelated refactors.
  - Add **no new dependencies**, including calendar libraries.
  - Report every deviation.
- **Public repo (AGENTS.md §11.2):** tests and docs use invented data only ("Ana Pérez", "Universidad Demo", "Álgebra Lineal", "Go to the doctor").
- **Layering:** `crates/core` is pure, with no I/O. Repositories live in `crates/infra/src/db/`. Commands and DTOs live in `src-tauri`. Frontend command calls go only through `src/lib/api/`.
- **Never write `lms_items` from personal-task code**, and never put personal tasks into it.
- **A personal task's day is a calendar date `YYYY-MM-DD`.**
  - Store it as text (`NaiveDate` in Rust, `string` in TS).
  - Never turn it into a UTC instant. In TS, never call `new Date('YYYY-MM-DD')`, because that parses as UTC midnight.
  - Today is the local date (`todayLocalIso()` in `src/components/results/format.ts`).
- **Validation (backend command layer)** has three rules:
  - **Title:** trimmed and required. An empty title gives "Enter a title". More than 200 characters gives "Titles can have at most 200 characters".
  - **Day:** must parse as `%Y-%m-%d`, otherwise "Enter a valid day".
  - **Note:** goes through the existing `validation::item_note` (trim, blank becomes none, at most 2000 characters).
- **UI copy:**
  - English, sentence case.
  - Weeks start on Monday.
  - Dates use the `en-GB` helpers in `format.ts`.
  - Reuse `Modal`, `ConfirmDialog`, `ErrorMessage`, `forms/*` and the current Tailwind look.
  - Every rejected command is shown inline with `toCommandError` and `ErrorMessage`.
- **Dot colours:**
  - red: overdue
  - blue: pending
  - green: submitted or graded
  - grey: past event
  - violet: personal (faded violet when done)
- **Logging:** never log or `console.*` titles or notes.
- **Gates before every commit:** `npm run typecheck`, `npm run lint` and `npm test`. Rust tasks also run `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings` and `cargo test --workspace`.
- **Git:** each task works on `feature/p3-t{N}-{slug}`, branched from `phase-3-calendar`. Commit there, and never merge or push. The commit trailer is `Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>`.

## Review Focus

1. **Day boundaries in local time.**
   - A personal task dated today is pending, never overdue, at any hour.
   - Yesterday's undone task is overdue.
   - A platform item due at 23:30 local time sits on that local day in the calendar, even when the UTC date differs.
   - Pinned in Task 4 (`personalStatus`, `localDayOf` tests) and Task 7 (`entriesByDay`).
2. **Month grid edges.**
   - November 2026 starts on a Sunday, so the grid shows 6 weeks starting Mon 26 Oct.
   - February 2027 starts on a Monday and has 28 days, so exactly 4 weeks.
   - Going › from December 2026 reaches January 2027.
   - Pinned in Task 7.
3. **Bad input when creating or editing.**
   - A whitespace-only title shows "Enter a title" inline and keeps the form.
   - A 201-character title shows the backend message.
   - Accented titles round-trip unchanged.
   - Pinned in Task 2 (validation) and Task 5 (modal keeps text on error).
4. **Deleting the task whose details are open.**
   - Cancel in the confirm dialog keeps it.
   - Confirm deletes it and closes the details; they do not reopen.
   - Pinned in Task 5.
5. **A crowded day.** More than 6 entries on one day shows 6 dots and "+N". The day list still shows all of them. Pinned in Task 7.

---

### Task 1: Personal task domain, port and SQLite repository

**Files:**
- Create: `crates/core/src/domain/personal.rs` (re-export it from `crates/core/src/domain/mod.rs`)
- Modify: `crates/core/src/ports/repositories.rs` (add the port)
- Create: `crates/infra/migrations/0004_personal_tasks.sql`
- Modify: `crates/infra/src/db/mod.rs` (`MIGRATIONS` becomes `[(i64, &str); 4]` with `(4, include_str!("../../migrations/0004_personal_tasks.sql"))`)
- Create: `crates/infra/src/db/personal.rs` (`impl PersonalTaskRepository for Database`, following `db/items.rs` style)
- Test: `crates/infra/tests/db_personal.rs` (follow `crates/infra/tests/db_items.rs` setup)

**Interfaces:**
- Produces, for Task 2:

```rust
// crates/core/src/domain/personal.rs
pub struct PersonalTask { pub id: i64, pub title: String, pub date: NaiveDate,
    pub note: Option<String>, pub done: bool,
    pub created_at: DateTime<Utc>, pub updated_at: DateTime<Utc> }   // Debug, Clone, PartialEq
pub struct PersonalTaskDraft { pub title: String, pub date: NaiveDate, pub note: Option<String> }

// crates/core/src/ports/repositories.rs
pub trait PersonalTaskRepository: Send + Sync {
    /// Every personal task, ordered by date, then id.
    fn list_personal_tasks(&self) -> RepoResult<Vec<PersonalTask>>;
    /// created_at = updated_at = now; done = false.
    fn insert_personal_task(&self, draft: &PersonalTaskDraft, now: DateTime<Utc>) -> RepoResult<PersonalTask>;
    /// Replaces title, date and note; keeps done. Missing id → RepoError::NotFound.
    fn update_personal_task(&self, id: i64, draft: &PersonalTaskDraft, now: DateTime<Utc>) -> RepoResult<PersonalTask>;
    /// Missing id → RepoError::NotFound.
    fn set_personal_task_done(&self, id: i64, done: bool, now: DateTime<Utc>) -> RepoResult<PersonalTask>;
    /// Missing id → RepoError::NotFound.
    fn delete_personal_task(&self, id: i64) -> RepoResult<()>;
}
```

- The migration has these columns:
  - `id INTEGER PRIMARY KEY`
  - `title TEXT NOT NULL`
  - `date TEXT NOT NULL` (`YYYY-MM-DD`)
  - `note TEXT`
  - `done INTEGER NOT NULL DEFAULT 0`
  - `created_at TEXT NOT NULL`
  - `updated_at TEXT NOT NULL`
- It also creates `CREATE INDEX idx_personal_tasks_date ON personal_tasks (date);`.
- Timestamps use the same RFC 3339 format and helpers as `db/items.rs` (`db/util.rs`).

- [ ] **Step 1: Write the failing tests** in `db_personal.rs`. Use invented titles such as "Go to the doctor" and "Lavar el coche — Álvaro".
  - `insert_then_list_orders_by_date_then_id`: insert dates 2026-11-10, 2026-11-05 and 2026-11-05. `list_personal_tasks` returns them in the order 05 (first id), 05 (second id), 10. `done` is false and `created_at == updated_at == now`.
  - `update_replaces_fields_and_keeps_done`: insert, `set_personal_task_done(id, true, t1)`, then `update_personal_task` with a new title, date and `note: None`. Expect the new fields, `done == true`, `updated_at == t2` and `created_at` unchanged.
  - `accented_title_round_trips`: the title "Recoger el DNI — Ñandú" comes back unchanged.
  - `missing_id_is_not_found`: update, set_done and delete on id 999 each return `Err(RepoError::NotFound)`.
  - `delete_removes_the_task`: after the delete, the list is empty.
  - `migration_applies_on_existing_database`: open a database that already has migrations 1–3 applied, using whatever helper `db_items.rs` or `db/mod.rs` tests use. After opening it with the new code, `list_personal_tasks` returns `Ok(vec![])`.
- [ ] **Step 2:** Run `cargo test -p academic-infra --test db_personal`. Expect a FAIL because the type or module is missing.
- [ ] **Step 3: Implement** the domain types, the port, the migration and the repository, mapping a missing row to `RepoError::NotFound`.
- [ ] **Step 4:** Run `cargo test -p academic-infra --test db_personal`, then all the Rust gates. Expect PASS.
- [ ] **Step 5: Commit** with the message `feat(db): add personal tasks table and repository`.

---

### Task 2: Personal task commands and `get_tracker` payload

**Files:**
- Modify: `src-tauri/src/state.rs` (add `pub personal_tasks: Arc<dyn PersonalTaskRepository>` to `AppState`, wired from `db` like `items`)
- Modify: `src-tauri/src/dto/lms.rs`
  - Add `PersonalTaskDto` and `PersonalTaskInput`.
  - Add `personal_tasks: Vec<PersonalTaskDto>` to `TrackerDto`; it serializes as `personalTasks`.
- Modify: `src-tauri/src/validation.rs` (add `personal_task(input) -> Result<PersonalTaskDraft, CommandError>` and `MAX_TITLE_CHARS: usize = 200`)
- Create: `src-tauri/src/commands/personal.rs` (register it in `commands/mod.rs` and in the `invoke_handler` list in `src-tauri/src/lib.rs`)
- Modify: `src-tauri/src/commands/tracker.rs` (`tracker()` fills `personal_tasks` from `state.personal_tasks.list_personal_tasks()`, regardless of `query.university_id`)

**Interfaces:**
- Consumes: Task 1 (`PersonalTaskRepository`, `PersonalTaskDraft`, `PersonalTask`).
- Produces, for Task 3 (exact names; arguments are camelCase on the JS side):

```rust
#[serde(rename_all = "camelCase")] pub struct PersonalTaskDto { id: i64, title: String, date: NaiveDate /* "YYYY-MM-DD" */, note: Option<String>, done: bool }
#[serde(rename_all = "camelCase")] pub struct PersonalTaskInput { title: String, date: String, note: Option<String> }

#[tauri::command] create_personal_task(input: PersonalTaskInput) -> Result<PersonalTaskDto, CommandError>
#[tauri::command] update_personal_task(id: i64, input: PersonalTaskInput) -> Result<PersonalTaskDto, CommandError>
#[tauri::command] set_personal_task_done(id: i64, value: bool) -> Result<PersonalTaskDto, CommandError>
#[tauri::command] delete_personal_task(id: i64) -> Result<(), CommandError>
```

- Each command wraps its body with the `logged("<name>", …)` helper, as `commands/tracker.rs` does. The `now` value comes from `state.clock.now()`.
- Error messages, exact:
  - "Enter a title"
  - "Titles can have at most 200 characters" (the limit counts `chars()`, not bytes)
  - "Enter a valid day"
  - Note errors come from `validation::item_note`.
- A missing id maps to `not_found` through the existing `From<RepoError>`.

- [ ] **Step 1: Write the failing tests** in `commands/personal.rs` with the `test_support` harness (`harness()`):
  - `create_trims_and_returns_the_task`: the input `{ title: "  Go to the doctor  ", date: "2026-11-05", note: Some("  ") }` returns the title "Go to the doctor", the date 2026-11-05, `note: None` and `done: false`.
  - `create_rejects_a_blank_title`: the title "   " gives `Validation` "Enter a title".
  - `create_rejects_a_201_character_title_but_accepts_200`: the title `"ñ".repeat(201)` gives "Titles can have at most 200 characters", and `"ñ".repeat(200)` is accepted.
  - `create_rejects_a_bad_day`: "2026-02-30" and "05/11/2026" each give "Enter a valid day".
  - `update_and_done_and_delete_round_trip`: create, update, `set_personal_task_done(true)` and delete. After the delete, a second delete gives `NotFound`.
  - In `commands/tracker.rs` tests, `tracker_includes_personal_tasks_for_every_tab`: with one personal task, `tracker()` with `university_id: None` and with `Some(id)` both return `personal_tasks.len() == 1`. Also with no universities at all, `tracker()` still returns the task.
- [ ] **Step 2:** Run `cargo test -p academic-assistant personal` (or the crate name used for `src-tauri`). Expect a FAIL.
- [ ] **Step 3: Implement.** Then run all the Rust gates and expect PASS.
- [ ] **Step 4: Commit** with the message `feat(app): add personal task commands`.

---

### Task 3: Frontend API for personal tasks

**Files:**
- Modify: `src/lib/api/types.ts`, `src/lib/api/client.ts`
- Test: `src/lib/api/client.test.ts`
- Create: `src/features/tracker/usePersonalTaskMutations.ts` (same pattern as `useItemMutations.ts`)

**Interfaces:**
- Consumes: Task 2 commands.
- Produces:

```ts
// types.ts
export interface PersonalTaskDto { id: number; title: string; date: string; note: string | null; done: boolean }
export interface PersonalTaskInput { title: string; date: string; note: string | null }
// TrackerDto gains: personalTasks: PersonalTaskDto[]

// client.ts (add to `api`)
createPersonalTask: (input: PersonalTaskInput) => invoke<PersonalTaskDto>('create_personal_task', { input }),
updatePersonalTask: (id: number, input: PersonalTaskInput) => invoke<PersonalTaskDto>('update_personal_task', { id, input }),
setPersonalTaskDone: (id: number, value: boolean) => invoke<PersonalTaskDto>('set_personal_task_done', { id, value }),
deletePersonalTask: (id: number) => invoke<void>('delete_personal_task', { id }),

// usePersonalTaskMutations.ts — each mutation invalidates the ['tracker'] prefix on success
export function usePersonalTaskMutations(): {
  create: UseMutationResult<PersonalTaskDto, unknown, PersonalTaskInput>;
  update: UseMutationResult<PersonalTaskDto, unknown, { id: number; input: PersonalTaskInput }>;
  setDone: UseMutationResult<PersonalTaskDto, unknown, { id: number; value: boolean }>;
  remove: UseMutationResult<void, unknown, number>;
};
```

- [ ] **Step 1: Write the failing tests.** Add one test per client method, asserting the exact command name and argument object (using the `mockedInvoke` style). Then update every existing `TrackerDto` fixture in the test suite to include `personalTasks: []`; `npm run typecheck` lists them.
- [ ] **Step 2:** Run `npm test -- src/lib/api`. Expect a FAIL.
- [ ] **Step 3: Implement.** Then run the gates and expect PASS.
- [ ] **Step 4: Commit** with the message `feat(ui): add personal task API client`.

---

### Task 4: Tracker entries and entry-based grouping

**Files:**
- Create: `src/features/tracker/entries.ts`, `src/features/tracker/entries.test.ts`
- Modify: `src/features/tracker/grouping.ts` and its test so they group entries
- Modify: `src/features/tracker/labels.ts` and `StatusPill.tsx` to add the `done` status
- Modify: `src/components/results/format.ts` and its test (`toLocalIsoDay`)
- Modify: `src/features/tracker/ItemGroups.tsx`
  - Minimal adaptation only: it keeps taking `items: LmsItemDto[]`.
  - It wraps them with `platformEntry` before grouping and unwraps them for `ItemRow`.
  - Task 5 replaces this adaptation.

**Interfaces:**
- Consumes: Task 3 types.
- Produces, for Tasks 5–7:

```ts
// format.ts
export function toLocalIsoDay(date: Date): string;            // local "YYYY-MM-DD"; todayLocalIso() now returns toLocalIsoDay(new Date())

// entries.ts
export type TrackerEntry =
  | { kind: 'platform'; key: string; item: LmsItemDto }        // key `platform-${item.id}`
  | { kind: 'personal'; key: string; task: PersonalTaskDto };  // key `personal-${task.id}`
export type EntryStatus = EffectiveStatus | 'done';
export function platformEntry(item: LmsItemDto): TrackerEntry;
export function personalEntry(task: PersonalTaskDto): TrackerEntry;
export function localDayOf(iso: string): string;               // local calendar day of an instant
export function personalStatus(task: PersonalTaskDto, today: string): 'pending' | 'overdue' | 'done';
export function entryStatus(entry: TrackerEntry, today: string): EntryStatus;   // platform: item.status
export function entryTitle(entry: TrackerEntry): string;
export function entryDay(entry: TrackerEntry): string | null;   // platform: localDayOf(dueAt) or null; personal: task.date
export function isEntryCompleted(entry: TrackerEntry, today: string): boolean;   // platform: submitted|graded|past_event; personal: done
export function compareEntries(a: TrackerEntry, b: TrackerEntry): number;

// grouping.ts: same group keys and labels, plus StatusGroupKey 'done' labelled "Done"
export type StatusGroupKey = 'pending' | 'submitted' | 'graded' | 'done' | 'past_events';
export interface ItemGroup<K extends string> { key: K; label: string; items: TrackerEntry[] }
export interface GroupedItems<K extends string> { groups: ItemGroup<K>[]; completed: TrackerEntry[] }
export function groupByTime(entries: TrackerEntry[], now: Date): GroupedItems<TimeGroupKey>;
export function groupByStatus(entries: TrackerEntry[], now: Date): GroupedItems<StatusGroupKey>;
// labels.ts: STATUS_LABELS: Record<EntryStatus, string> adds done: 'Done'. StatusPill takes EntryStatus; 'done' uses the submitted/graded green style.
// Remove isCompleted/compareItems from grouping.ts; their callers move to isEntryCompleted/compareEntries.
```

Rules:
- `personalStatus`: `done` gives `'done'`. Otherwise `date < today` (a string compare of ISO days) gives `'overdue'`, and anything else gives `'pending'`.
- Time grouping for a personal task works on its day string versus `toLocalIsoDay` of the same boundaries the platform code computes (tomorrow, next Monday, the Monday after):
  - overdue
  - today
  - this week
  - next week
  - later
  - Personal tasks never land in "No date".
  - Done ones go to `completed`.
- Status grouping:
  - **Pending:** platform pending and overdue, plus personal pending and overdue, with overdue first.
  - **Submitted** and **Graded:** platform items, as today.
  - **Done:** personal tasks marked done.
  - **completed:** platform past events.
- `compareEntries` sort key:
  - A platform item sorts by `Date.parse(dueAt)`.
  - A personal task sorts by its day's local midnight (`new Date(y, m - 1, d)`), so it comes before that day's timed deadlines.
  - Undated entries go last.
  - Ties break on the locale-aware title (the existing collator), then `key`.

- [ ] **Step 1: Write the failing tests.**
  - **Port the existing `grouping.test.ts` cases:** wrap each item with `platformEntry`. The expectations must not change, apart from the `isCompleted` → `isEntryCompleted` rename.
  - **Add to `entries.test.ts`.** Each test name below is followed by its exact assertions.
    - `personal task dated today is pending at any hour`: `personalStatus(task('2026-11-05'), '2026-11-05') === 'pending'`.
    - `undone task from yesterday is overdue`: `personalStatus(task('2026-11-04'), '2026-11-05') === 'overdue'`, and with `done: true` it is `'done'`.
    - `localDayOf uses the local day`: `localDayOf(new Date(2026, 10, 5, 23, 30).toISOString()) === '2026-11-05'` and `localDayOf(new Date(2026, 10, 6, 0, 10).toISOString()) === '2026-11-06'`.
    - `personal task sorts before a timed deadline the same day`: a platform item due `new Date(2026, 10, 5, 9, 0)` and a personal task on '2026-11-05' sort as [personal, platform].
  - **Add to `grouping.test.ts`.** Fix `now = new Date(2026, 10, 4, 10, 0)` (Wednesday 4 Nov 2026). Time grouping places personal tasks dated:
    - '2026-11-03' (undone) → overdue
    - '2026-11-04' → today
    - '2026-11-08' (Sunday) → this_week
    - '2026-11-09' (Monday) → next_week
    - '2026-11-16' → later
    - done → `completed`
  - **Status grouping:** a done personal task lands in the 'done' group, labelled "Done", placed after 'graded'.
  - **`toLocalIsoDay`:** `toLocalIsoDay(new Date(2026, 0, 9)) === '2026-01-09'`.
- [ ] **Step 2:** Run `npm test -- src/features/tracker src/components/results`. Expect a FAIL.
- [ ] **Step 3: Implement.** Then run the gates and expect PASS (the existing ItemGroups and TrackerPage tests still pass).
- [ ] **Step 4: Commit** with the message `feat(ui): group platform items and personal tasks as tracker entries`.

---

### Task 5: Entry rows, personal task modal and shared details

**Files:**
- Create:
  - `src/features/tracker/PersonalTaskRow.tsx`
  - `src/features/tracker/EntryRow.tsx` (dispatches to `ItemRow` or `PersonalTaskRow`)
  - `src/features/tracker/PersonalTaskModal.tsx` (create and edit)
  - `src/features/tracker/EntryDetailsModal.tsx` (dispatches to `ItemDetailsModal` or `PersonalTaskModal`)
  - `src/features/tracker/useOpenEntry.ts`
  - Tests: `PersonalTaskRow.test.tsx`, `PersonalTaskModal.test.tsx`, `useOpenEntry.test.ts`
- Modify: `src/features/tracker/ItemGroups.tsx` (now takes `entries: TrackerEntry[]` and `onOpen(entry)`, renders `EntryRow`) and its test; `src/features/tracker/TrackerPage.tsx` only as much as the new `ItemGroups` props require (pass `items.map(platformEntry)`). Task 6 does the rest.

**Interfaces:**
- Consumes: Tasks 3 and 4.
- Produces, for Tasks 6 and 7:

```ts
export interface EntryRowProps { entry: TrackerEntry; showUniversity: boolean; onOpen: (entry: TrackerEntry) => void }
export interface PersonalTaskModalProps {
  open: boolean;
  task: PersonalTaskDto | null;   // null → create mode
  initialDate: string;            // create mode default day (YYYY-MM-DD)
  onClose: () => void;
}
export interface EntryDetailsModalProps { entry: TrackerEntry | null; onClose: () => void }
// Keeps the selected entry key; returns the live entry from `entries`; forgets the key once fresh
// (non-placeholder) data no longer contains it, so it can't reopen (same rule as TrackerPage's openItemGone today).
export function useOpenEntry(entries: TrackerEntry[] | undefined, isPlaceholderData: boolean):
  { openEntry: TrackerEntry | null; open: (entry: TrackerEntry) => void; close: () => void };
```

Requirements:
- **`PersonalTaskRow`** (an `li`, laid out like `ItemRow`):
  - The title is a button that calls `onOpen`.
  - Muted text reads "Personal".
  - The status pill shows `entryStatus` (Pending, Overdue or Done).
  - The day is shown as `formatResultDate(task.date, null)`, the existing local-date formatter.
  - A done checkbox has `aria-label="Mark {title} as done"`, bound to `task.done`, and calls `setDone`.
  - A note indicator (`role="img"`, `aria-label="Has a note"`) shows when there is a note.
  - Mutation errors appear inline.
  - Reuse `ItemRow`'s class constants by exporting them from a shared module. Do not copy the strings.
- **`PersonalTaskModal`** (`Modal`):
  - **Title:** "Add task" in create mode, "Edit task" in edit mode.
  - **Fields:**
    - Title (`TextInput`, label "Title", required)
    - Day (`input type="date"`, label "Day", value `YYYY-MM-DD`, default `initialDate` in create mode)
    - Note (`Textarea`, label "Note")
  - **Saving:**
    - "Save" calls `create` or `update` with `note: text.trim() === '' ? null : note`.
    - On success the modal closes. On error it shows the backend message inline and keeps every field.
  - **Edit mode extras:**
    - "Mark as done" / "Mark as not done" calls `setDone`.
    - "Delete" opens `ConfirmDialog`, titled "Delete task?", with the message "This deletes the task and its note. This cannot be undone." and the confirm label "Delete".
    - Confirm calls `remove(id)` and then `onClose`. Cancel keeps everything.
  - Key the inner form by task id, or by `'new'`, so switching tasks resets the fields.
- **`EntryDetailsModal`:** a platform entry renders `ItemDetailsModal` (unchanged), and a personal one renders `PersonalTaskModal` in edit mode with `open = entry !== null`.

- [ ] **Step 1: Write the failing tests.** Mock `api` and wrap in `QueryClientProvider`.
  - `PersonalTaskRow`:
    - renders the title, "Personal", the day and the "Overdue" pill for a past undone task;
    - the checkbox calls `setPersonalTaskDone(id, true)`;
    - the note indicator shows only with a note.
  - `PersonalTaskModal`, create mode:
    - the day defaults to `initialDate`;
    - saving "  Go to the doctor  " calls `createPersonalTask({ title: '  Go to the doctor  ', date: '2026-11-05', note: null })`, because trimming is the backend's job;
    - a rejected create with `{ kind: 'validation', message: 'Enter a title' }` shows that message and keeps the typed text.
  - `PersonalTaskModal`, edit mode:
    - fields are prefilled;
    - "Delete" then "Cancel" does not call `deletePersonalTask`;
    - "Delete" then "Delete" calls `deletePersonalTask(id)` and `onClose`.
  - `useOpenEntry`:
    - `open(e)` returns the live entry with updated fields after `entries` changes;
    - when fresh data lacks the key, `openEntry` becomes null and stays null when the entry comes back (Review Focus 4);
    - while `isPlaceholderData` is true, the key is kept.
  - `ItemGroups.test.tsx`: existing cases are passed through `platformEntry`, and a personal entry renders through `EntryRow` as "Personal".
- [ ] **Step 2:** Run `npm test -- src/features/tracker`. Expect a FAIL.
- [ ] **Step 3: Implement.** Then run the gates and expect PASS.
- [ ] **Step 4: Commit** with the message `feat(ui): add personal task rows and editor`.

---

### Task 6: Personal tasks on the dashboard

**Files:**
- Modify: `src/features/tracker/TrackerPage.tsx` and `TrackerPage.test.tsx`

**Interfaces:**
- Consumes: Tasks 4 and 5 (`platformEntry`, `personalEntry`, `ItemGroups` with entries, `useOpenEntry`, `EntryDetailsModal`, `PersonalTaskModal`).

Requirements:
- **Which entries:** on the All tab, `items.map(platformEntry)` plus `personalTasks.map(personalEntry)`. On a university tab, platform entries only.
- **Selection:** replace `openItemId`/`openItemGone` with `useOpenEntry(entries, isPlaceholderData)` and `EntryDetailsModal`.
- **"Add task" button:** shown next to Sync on the **All tab only**. It opens `PersonalTaskModal` in create mode with `initialDate = todayLocalIso()`.
- **Unchanged:** the two early empty states ("Track your deliverables" and "No courses are tracked yet") stay as they are, with no personal tasks and no "Add task" (spec §5).
- **Empty list:** the "Nothing to show…" text counts entries, so a dashboard with only personal tasks lists them.

- [ ] **Step 1: Write the failing tests** in `TrackerPage.test.tsx`:
  - the All tab shows a personal task "Go to the doctor" with "Personal", and the UAH tab does not;
  - "Add task" is visible on All and absent on the UAH tab;
  - clicking it shows the "Add task" dialog with the day input set to today (`todayLocalIso()`);
  - clicking a personal task title opens "Edit task";
  - with zero universities the first-run card shows and "Add task" is absent.
- [ ] **Step 2:** Run `npm test -- src/features/tracker/TrackerPage`. Expect a FAIL.
- [ ] **Step 3: Implement.** Then run the gates and expect PASS.
- [ ] **Step 4: Commit** with the message `feat(ui): show and add personal tasks on the dashboard`.

---

### Task 7: Calendar page

**Files:**
- Create:
  - `src/features/calendar/monthGrid.ts` and `monthGrid.test.ts`
  - `src/features/calendar/CalendarPage.tsx` and `CalendarPage.test.tsx`
  - `src/features/calendar/CalendarGrid.tsx`
  - `src/features/calendar/DayList.tsx`
- Modify:
  - `src/App.tsx`: add the route `/calendar`.
  - `src/components/Layout.tsx`: add the nav item "Calendar" right after Dashboard, then update `Layout.test.tsx`'s label list.
  - `src/components/navIcons.tsx`: add `CalendarIcon`, a simple inline SVG like the others.
  - `src/components/results/format.ts` and its test: add `formatLongDay` and `formatMonthYear`.

**Interfaces:**
- Consumes: Task 3 (`api.getTracker`), Task 4 (entries helpers), Task 5 (`EntryRow`, `EntryDetailsModal`, `useOpenEntry`, `usePersonalTaskMutations`).
- Produces:

```ts
// monthGrid.ts
export interface CalendarDay { date: string; inMonth: boolean }       // date: local "YYYY-MM-DD"
export function monthGrid(year: number, month: number): CalendarDay[][]; // month 0–11; Monday-first weeks covering the whole month; 4–6 rows
export function entriesByDay(entries: TrackerEntry[]): Map<string, TrackerEntry[]>; // key = entryDay(entry); undated entries omitted; each list sorted with compareEntries
export type DotTone = 'overdue' | 'pending' | 'completed' | 'past' | 'personal' | 'personal_done';
export function dotTone(entry: TrackerEntry, today: string): DotTone;  // personal → personal / personal_done; else by status (submitted|graded → completed, past_event → past)
export const MAX_DOTS = 6;
// format.ts
export function formatLongDay(isoDay: string): string;   // "Tuesday 10 November": built from en-GB formatToParts as `${weekday} ${day} ${month}`
export function formatMonthYear(year: number, month: number): string; // "November 2026"
```

Requirements:
- **Data:** `useQuery({ queryKey: queryKeys.tracker(ALL), queryFn: () => api.getTracker(ALL) })` with `ALL = { universityId: null, includeHidden: false, includeRemoved: false }`. This is the same key as the dashboard's All tab, so the cache is shared. Entries are `items.map(platformEntry)` plus `personalTasks.map(personalEntry)`. The page works with zero universities.
- **Header:** `h1` "Calendar" (`text-2xl`, like the other pages).
  - Buttons: "Previous month" (‹, `aria-label`), `formatMonthYear`, "Next month" (›, `aria-label`) and "Today" (jumps to the current month and selects today).
  - The page starts on the current month with today selected.
- **Grid** (`role="grid"` is optional; the tests use the buttons):
  - The header row reads Mon to Sun.
  - Each day is a button with `aria-label={formatLongDay(date)}` and `aria-pressed` when selected.
  - Days outside the month are muted. Today is outlined (`aria-current="date"`).
  - Up to `MAX_DOTS` dots follow, then "+N" muted text.
  - Each dot is a `span` with `title={entryTitle(entry)}` and `aria-label={entryTitle(entry)}`, coloured per `dotTone`:
    - overdue: red-600
    - pending: blue-600
    - completed: green-600
    - past: neutral-400
    - personal: violet-600
    - personal_done: violet-300
- **Legend under the grid:** Overdue, Pending, Submitted / graded, Past event, Personal.
- **`DayList`, under the legend** (layout B):
  - The heading is `formatLongDay(selected)`.
  - Rows are the day's entries rendered with `EntryRow` (`showUniversity = true`), and clicking one opens `EntryDetailsModal` via `useOpenEntry`.
  - An empty day shows "Nothing on this day."
  - The add box is a text input (`aria-label="Add a task for this day"`) plus an "Add" button. It calls `create({ title, date: selected, note: null })`. On success the input is cleared, and on error the message shows inline with the text kept.

- [ ] **Step 1: Write the failing tests.**
  - `monthGrid.test.ts`:
    - `monthGrid(2026, 10)` (November 2026) has 6 rows. The first day is `{ date: '2026-10-26', inMonth: false }`, 1 Nov is `[0][6]`, and the last is '2026-12-06'.
    - `monthGrid(2027, 1)` has exactly 4 rows, from '2027-02-01' to '2027-02-28', all `inMonth`.
    - `entriesByDay` places a platform item due `new Date(2026, 10, 5, 23, 30)` on '2026-11-05' and omits an undated item.
    - `dotTone` returns:
      - `personal_done` for a done personal task;
      - `completed` for a graded item;
      - `overdue` for an overdue one;
      - `past` for a past event.
  - `CalendarPage.test.tsx`: mock `api` and use a `MemoryRouter`. Use dates relative to the real current month so the test is stable, computing ISO days with `toLocalIsoDay`.
    - Today's button has `aria-current="date"` and `aria-pressed="true"`, and the heading equals `formatLongDay(today)`.
    - A day with 8 entries shows 6 dots and "+2", and selecting it lists all 8 rows (Review Focus 5).
    - Dot `title` attributes equal entry titles.
    - "Next month" from December reaches January of the next year. Render December by navigating, or by clicking "Next month" until the heading matches (Review Focus 2).
    - Typing "Go to the doctor" and clicking "Add" calls `createPersonalTask({ title: 'Go to the doctor', date: <selected day>, note: null })`, and the input clears.
    - With `universities: []` and one personal task, the page renders the task's dot (it does not show the first-run card).
  - `Layout.test.tsx`: the labels become `['Dashboard', 'Calendar', 'Announcements', 'Universities', 'Email accounts', 'Email runs', 'Settings']`.
  - `format` test:
    - `formatLongDay('2026-11-10') === 'Tuesday 10 November'`;
    - `formatMonthYear(2026, 10) === 'November 2026'`.
- [ ] **Step 2:** Run `npm test -- src/features/calendar src/components`. Expect a FAIL.
- [ ] **Step 3: Implement.** Then run the gates and expect PASS.
- [ ] **Step 4: Commit** with the message `feat(ui): add the calendar page`.

---

### Task 8: Documentation and acceptance checklist

**Files:**
- Modify: `docs/frontend/tracker-universities.md` (personal tasks in the dashboard, "Add task", entries), `docs/user-guide.md` (Calendar section and personal tasks), `docs/app/tauri-commands.md` (the four new commands plus `TrackerDto.personalTasks`), `docs/infra/database.md` (migration 0004, `personal_tasks`), `docs/testing.md` (new "Calendar and personal tasks acceptance checklist"), `docs/frontend/overview.md` (routes and nav)
- Modify: the spec, appending "§11 As built" with any deviations recorded during execution. Use invented values only.

Requirements:
- The checklist covers:
  - adding a task from the calendar day list and from the dashboard;
  - a task dated today being pending and yesterday's being overdue;
  - editing the title, day and note;
  - mark done from the row and from the calendar;
  - delete with cancel and confirm;
  - dot colours and tooltips;
  - a crowded day's "+N";
  - month navigation across the year;
  - personal tasks only on the All tab;
  - the calendar working with no university.
- All examples are invented.
- Run `git diff --check`.
- [ ] **Step 1:** Write the docs. **Step 2:** Run `git diff --check`. **Step 3:** Commit with the message `docs: describe the calendar and personal tasks`.

## Task order

`T1 → T2 → T3 → T4 → T5 → T6 → T7 → T8`, strictly sequential.
