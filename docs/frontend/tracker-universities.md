# Dashboard (tracker), Calendar, Universities and sync state

The Phase 2 and 3 screens built on the API client from `src/lib/api/` (see [overview.md](overview.md)):
the tracker **Dashboard** (`/`), the **Calendar** (`/calendar`) with the user's personal tasks, the
**Universities** page (`/universities`) with its add wizard and
course selection, and the app-level sync state. The backend side is documented in
[../infra/lms.md](../infra/lms.md); the commands in
[../app/tauri-commands.md](../app/tauri-commands.md). Email lives in a
separate section, Announcements ([dashboard-results-runs.md](dashboard-results-runs.md)).

Examples are invented (`Universidad Demo`, `Álgebra Lineal`, "Go to the doctor").

## Sync state (`src/features/sync/`)

`SyncProvider` is mounted once in `src/main.tsx` (inside `QueryClientProvider`, next to
`CheckRunProvider`) and `useSync()` returns:

| Field | Meaning |
|---|---|
| `running` | True from a successful `startSync` until `lms-sync:finished`; also true as soon as a progress event arrives (a sync started elsewhere or already running) |
| `progress` | Label of the latest progress event while running, e.g. `Universidad Demo: Fetching items` (stages: Checking courses, Fetching items, Saving, Done, Failed, Sign-in required) |
| `lastResults` | Results of the last finished sync; an empty array after an interrupted sync; `null` before any |
| `error` | Error of the last `startSync` call, such as the `conflict` "A sync is already running" |
| `startSync(universityId \| null)` | Calls `lms_sync`; `null` syncs every signed-in university. It never throws: rejections are stored in `error` |
| `clearResults()` | Closes the summary |

It subscribes to `lms-sync:progress` and `lms-sync:finished` for the whole app lifetime, so a sync
that finishes before the `lms_sync` call returns is never missed. On `lms-sync:finished` it
invalidates the `['tracker']`, `['universities']` and `['courses']` query prefixes, so every route
reacts to a finished sync.

## Dashboard (`src/features/tracker/`)

`TrackerPage` calls `getTracker({ universityId, includeHidden, includeRemoved })` and keeps the
previous data while a new query loads. What it renders depends on the data:

- No universities: a card "Track your deliverables" with **Add your first university**, which opens
  the Universities page with the add wizard already open (`state: { openAdd: true }`).
- Universities but no tracked course: "No courses are tracked yet." with **Choose courses**. On
  the tab of a single university with no tracked course (and no items), the list is replaced by
  "No courses are tracked for <name>." with the same button.
- Otherwise the dashboard:
  - Header with **Sync** (the selected university, or all on the "All" tab; shows progress while
    running) and "Last synced <day> at <time>" (the latest sync of the universities in view). A failed
    `startSync` is shown inline. On the **All** tab only, an **Add task** button next to Sync opens
    the personal task form (see "Personal tasks" below).
  - Tabs: **All** plus one per university. Tab and grouping are stored in `localStorage`
    (`academic-assistant.tracker-tab`, `academic-assistant.tracker-group`) through
    `usePersistentState`, with every access in try/catch and a validated fallback. A stored tab
    whose university no longer exists falls back to All.
  - **Group by: Time | Status**.
  - **Show hidden** and **Show removed** checkboxes (off by default; they change the query).
  - `SyncBanners`, then `ItemGroups`, then the details modal (`EntryDetailsModal`) and the
    **Add task** modal.

The list works on **entries** (`entries.ts`), a discriminated union: `{ kind: 'platform', key,
item }` or `{ kind: 'personal', key, task }`, with keys `platform-<id>` and `personal-<id>`.
`TrackerPage` builds them from `TrackerDto.items` and, **only on the All tab**,
`TrackerDto.personalTasks`; a university tab lists platform items only. `entries.ts` also holds the
shared helpers `entryStatus`, `entryTitle`, `entryDay`, `isEntryCompleted`, `localDayOf` and the
sort `compareEntries` (a personal task sorts as local midnight of its day, so it precedes that
day's deadlines). The two empty states above (no universities, no tracked course) are unchanged
and show neither personal tasks nor **Add task**; use the Calendar there.

### Grouping (`grouping.ts`)

Pure functions, tested with a fixed `now`.

- **Time** (local time, weeks start on Monday): Overdue, Today, This week, Next week, Later, No
  date. Items with no date go to No date; overdue items to Overdue regardless of date. A personal
  task is placed by comparing its calendar day with local dates, never as an instant. Completed
  entries (submitted, graded, past event, done personal task) leave these groups and appear in a
  collapsed **Completed (n)** section. `ItemGroups` renders every group as a `<details>` drop-down (open by
  default, state not persisted; the h2 heading sits inside the `<summary>`), so any section can be
  collapsed.
- **Status**: Pending (overdue first, personal tasks included), Submitted, Graded, **Done** (done
  personal tasks, a visible group after Graded); past events in a collapsed **Past events (n)**
  section, which stays platform-only.
- `groupByTime(entries, now)` and `groupByStatus(entries, now)` both take `now`, and "today" is
  its local date, so the tests pin the day boundaries.
- Inside each group, entries are sorted by date ascending, undated last, then by title with a
  locale-aware collator, then key.

### Rows and details

- `EntryRow` renders an entry with `ItemRow` (platform) or `PersonalTaskRow` (personal). Both use
  the class names in `rowStyles.ts` and the pencil mark in `NoteIndicator.tsx`, so the two look
  alike.
- `ItemRow`: university chip (only on the All tab), title (opens the details), a note indicator,
  course short name (or full name) and kind, the status pill (`StatusPill`: Pending, Overdue,
  Submitted, Graded, Past), the grade text, the due date and time or "No date", the change badge
  with a dismiss button ("New", or "Date changed: 5 Oct → 12 Oct"), the "Hidden" (grey, with a
  "×" button that unhides the item) and "Removed from platform" (red, no button) chips, and
  the **done** checkbox (shown only for non-event items that are still pending or overdue on the
  platform, or that you already ticked, so a done item can always be unticked). Errors from the row's mutations show inline.
- `ItemDetailsModal`: university, course and kind; status and grade; due date; change text;
  "Removed from the platform on <date>"; **Open in platform** (only when the item's URL starts with
  `https://`, opened with `openUrl` from `@tauri-apps/plugin-opener`; a failure shows "Couldn't
  open the link"); a note editor (**Save note**; an empty or whitespace note clears it; over 2000
  characters shows the backend's validation message and keeps the text); **Hide** / **Show again**;
  **Mark as done** / **Mark as not done** (same rule as the row checkbox).
- `useItemMutations` wraps `set_item_done`, `set_item_hidden`, `set_item_note` and
  `dismiss_item_change`; each invalidates the `['tracker']` prefix.
- `EntryDetailsModal` opens `ItemDetailsModal` for a platform entry and `PersonalTaskModal` for a
  personal one. `useOpenEntry` remembers the open entry by key and returns the live entry, so
  edits show while it is open; once fresh data no longer contains the key (the task was deleted)
  the key is forgotten and the details never reopen.

### Personal tasks (`PersonalTaskRow`, `PersonalTaskModal`)

A personal task is a one-off to-do for one day (title, day `YYYY-MM-DD`, optional note) that the
user creates. It lives in its own table (`personal_tasks`), never in `lms_items`, so a sync cannot
touch it. It has no hidden state and no badges.

- `PersonalTaskRow`: the **done** checkbox (`set_personal_task_done`), the title (opens the
  editor), a note indicator, "Personal" in place of course and kind, a status pill (Pending,
  Overdue or Done) and the day. A task dated today is **Pending** at any hour; an undone task
  dated before today is **Overdue** (`personalStatus(task, today)` compares the day text with the
  local `todayLocalIso()`).
- `PersonalTaskModal` has two modes, with a form that remounts per task (`key`):
  - **Add task** (`task = null`): Title, Day (a date input, today by default) and Note; **Save**
    calls `create_personal_task`. (The calendar adds through its own day box instead.)
  - **Edit task**: the same fields prefilled, **Save**, **Mark as done** / **Mark as not done**
    and **Delete**. **Delete** opens a `ConfirmDialog` ("Delete task?"); Cancel keeps the task,
    Confirm calls `delete_personal_task` and closes the details.
  - A rejected command (for example a whitespace-only title: "Enter a title") is shown inline
    with `toCommandError` and `ErrorMessage`, and the form keeps what was typed.
- `usePersonalTaskMutations` wraps the four commands; each invalidates the `['tracker']` prefix, so
  the dashboard and the calendar refresh together.

## Calendar (`src/features/calendar/`)

`CalendarPage` (route `/calendar`, sidebar item after Dashboard) shows every dated entry of every
university plus the personal tasks, without tabs.

- **Data**: the same query as the dashboard's All tab,
  `getTracker({ universityId: null, includeHidden: false, includeRemoved: false })`, so both screens
  share one cache entry. Hidden and removed platform items are not shown, nor are items without a
  due date. A platform item sits on the **local** calendar day of its `dueAt` (an item due at
  23:30 local time stays on that day whatever the UTC date); an overdue item only on its due day.
  The page works with no university at all (personal tasks only).
- **Header**: **‹** (Previous month), the month and year, **›** (Next month) and **Today**. Months
  roll over the year (› from December 2026 shows January 2027). **Today** shows the current month
  and selects today. The page opens on the current month with today selected.
- **Grid** (`CalendarGrid`, `monthGrid.ts`): Monday to Sunday, 4 to 6 weeks (`monthGrid(year,
  month)`: November 2026 starts on a Sunday, so it shows 6 weeks from Monday 26 October; February
  2027 starts on a Monday and shows exactly 4). Days of neighbouring months are muted, today is
  outlined and the selected day is highlighted. Each day is a button named after its long date,
  with one dot per entry in the order of `compareEntries`.
- **Dots** (`dotTone`): red overdue, blue pending, green submitted or graded, grey past event,
  violet personal (faded violet when done). Overdue applies to platform items only: a personal
  task stays violet and shows "Overdue" in its row. Hovering a dot shows the entry's full title
  (`title` and `aria-label`). A day shows at most `MAX_DOTS` = 6 dots, then "+N" for the rest;
  the day list still shows every entry. A legend under the grid names the colours.
- **Day list** (`DayList`): the heading of the selected day (for example "Tuesday 10 November"),
  its rows (the same `EntryRow` as the dashboard, with the done checkbox; "Nothing on this day."
  when empty) and an "Add a task for this day" box with **Add**, which creates a personal task
  for that day and clears the box. A rejected command shows inline and keeps the text. Clicking a
  row opens the same `EntryDetailsModal` as the dashboard.
- **Selection**: moving to another month keeps the selected day (the list keeps showing it), and
  clicking a muted day of a neighbouring month selects it without changing the viewed month.

### Banners (`SyncBanners.tsx`)

- For each shown university whose login state is `expired`, or whose last sync needed a login and
  whose login state is not `ok` (so the banner goes away once the connection works again):
  "<name> needs you to sign in again" with **Sign in** (`lms_sign_in`, then a sync of that
  university).
- After a sync: "Sync finished: 3 new, 1 date changed, 1 removed", a red line per university that
  did not succeed (`<name>: <error>`), **Dismiss all changes** (`dismiss_all_changes(null)`) and
  **Close**. After an interrupted sync: "The sync was interrupted. Try again."

## Universities (`src/features/universities/`)

`UniversitiesPage` lists the universities (`getUniversities`) as `UniversityCard`s and opens the
add wizard (it clears the `openAdd` navigation state once consumed, so going back does not reopen
the wizard). With no universities it shows "No universities yet" and **Add university**. A sync
start error (for example "A sync is already running" after **Save** in Edit courses) shows at the
top of the page.

### `UniversityCard`

Shows the name, platform and URL, the connection line ("Connected as <name>", "Not signed in",
"Session expired" or "Sign-in failed"), "<n> of <m> courses tracked", an "<n> new courses found"
(or "1 new course found") link (opens Edit courses; shown when some, but not all, courses are unreviewed), the last sync
time and, when the last sync failed, its error. Actions by login state:

| State | Actions |
|---|---|
| `ok` | **Sync**, **Edit courses**, **Check connection**, **Sign out** |
| `never` | **Sign in** |
| `expired`, `error` | **Sign in**, **Check connection**, **Sign out** |
| any | Rename (pencil), Delete (trash) |

Delete asks for confirmation ("Deletes this university, its courses, their items and your notes, and your saved sign-in. This cannot be undone."). On
the backend the delete also removes the university's stored items and notes (cascade) and closes
any open sign-in window. A failed delete shows its error inside the dialog. Rename uses
`RenameUniversityModal`; the platform and URL cannot be changed (delete and add again).

### Add university wizard (`AddUniversityModal`)

1. **Details**: name, platform (Moodle or Blackboard) and platform URL. **Continue** calls
   `add_university`, which normalizes and probes the URL and rejects a mismatch or a duplicate
   ("This platform is already added"). Closing the wizard after this step keeps the university,
   listed as "Not signed in".
2. **Sign in**: opens the university's sign-in window; on success the dialog shows "Connected as
   <name>" and **Choose courses** loads them (`lms_fetch_courses`).
3. **Courses**: `CourseSelectionList` with every course ticked. **Save and sync** stores the
   selection (`set_tracked_courses`), starts a sync of the university and navigates to the
   Dashboard.

### Edit courses (`EditCoursesModal`)

Refreshes the list from the platform (`lms_fetch_courses`). If that fails (for example the session
expired) it shows the stored list (`get_courses`) together with the error above it. **Save** stores
the selection and starts a sync of that university; a sync problem is reported at the top of the
Universities page (and on the Dashboard), not by the modal. The list lives under
`queryKeys.courses(id)`; saving drops it before the `['courses']` invalidation so the platform is
fetched once per open. A failed refresh also invalidates `['universities']`, so the card shows the
new login state even if the modal is cancelled.

`CourseSelectionList` is a checkbox list sorted by name with **Select all** / **Select none**, the
short name when it differs, a **New** tag for courses the user has not reviewed yet, and "Not
visible on the platform" for unavailable ones. Saving marks every course of the university as
reviewed.

`useInvalidateUniversities` refreshes `['universities']`, `['catalog']` and `['courses']` after any
university or course change.

## Navigation (`src/components/Layout.tsx`)

The sidebar items, in order: **Dashboard** (`/`), **Announcements** (`/announcements`),
**Universities**, **Email accounts** (`/accounts`), **Email runs** (`/runs`), **Settings**.
Announcements is rendered disabled, with the tooltip "Add a university and track courses first",
while the universities query has loaded and no course is tracked. The unseen-results badge on
Announcements comes from `getDashboard('upcoming')`. When a query fails (for example rendered
outside Tauri) the layout still renders, with Announcements enabled and no badge.

## Testing

Vitest with the API client, the events module and `@tauri-apps/plugin-opener` mocked. Covered:
the API client's command names and arguments; `SyncProvider` (events, a fast finish, `conflict`
errors); time and status grouping around week boundaries; `ItemRow` and `ItemDetailsModal`
actions and the note editor; the wizard steps, Edit courses with fallback, the card actions and
the page empty states; the tracker page states, tabs, persistence and banners.

## Known limitations

- No month calendar view and no export; the dashboard is a grouped list.
- The Dashboard "Last synced" line shows the latest sync time among the universities in view, not
  one per university (the Universities page shows each).
- Sync is manual; there is no background refresh.
