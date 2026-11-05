# Calendar and personal tasks: design

Date: 2026-11-03 · Status: approved; implemented (see §11) · Builds on Phase 2 (LMS tracker, `v0.1.0`).

## 1. Intent

The dashboard lists what the platforms (Moodle, Blackboard) say is due. The user also wants to plan their own day-to-day errands in the same place, like "wash the car" or "go to the doctor", and to see everything on a month calendar. This adds:

1. **Personal tasks:** one-off to-dos the user creates for a given day.
2. **A Calendar section** in the sidebar: a month view with one coloured dot per item and per personal task, and a list for the selected day where tasks can be added.

Success: the user opens Calendar, sees at a glance which days are busy and which deadlines are overdue, clicks a day, adds "Go to the doctor", and later ticks it off from either the calendar or the dashboard.

## 2. Goals

- Create, rename, move to another day, add a note to, mark done and delete personal tasks.
- Create them from the Calendar (the selected day's "Add a task for this day" box) and from the Dashboard (an "Add task" button).
- Show personal tasks in the dashboard list next to platform items, with the same done checkbox.
- The Calendar shows every university's items plus personal tasks, all at once, without tabs.

## 3. Non-goals

- No times on personal tasks. A task has a day only.
- No recurring tasks: no "every day" or "every week".
- No reminders or notifications.
- No separate day notes. "A note for a day" is a personal task on that day.
- No Google Calendar export. That is a later phase, and stable task ids keep it possible.
- No week or day views; the month view only.

## 4. Anti-goals: failures even if it works

- **The sync must never be able to touch personal tasks.** Personal tasks must not be stored in `lms_items`. That table requires a university and a course, and the sync owns its rows: it upserts them, marks them removed and dismisses their changes. This is the user's explicit decision.
- **No duplicated logic.** There must not be one grouping, row or details implementation for platform items and a second, copied one for personal tasks.
- **No wrong days around midnight.** A personal task on 5 Nov must never appear on 4 or 6 Nov. It must never be "overdue" on its own day, for any time zone.
- **No real data in the repo.** The repo is public (AGENTS.md §11.2), so tests and docs use invented data only.

## 5. Decisions the user made

| Topic | Decision |
|---|---|
| What a personal task is | A one-off to-do for one particular day, with no time and no recurrence. |
| Fields | A title and a day, plus an optional note using the same note box as platform items. |
| Clicking a day in the calendar | Selects it, and the day's list lets the user add a personal task for that day. |
| Storage | A separate table for personal tasks; the platform items table is untouched. The two kinds meet only in the UI. |
| Calendar scope | All universities and personal tasks at once, without tabs. |
| Calendar layout | Option B: the month grid on top, with the selected day's list and its "Add a task for this day" box underneath. |
| Dot colours | Red = overdue, blue = pending, green = submitted or graded, grey = past event, violet = personal. |
| Dot tooltip | Hovering a dot shows its title. |
| Dashboard creation | An "Add task" button next to Sync opens a small form with a title, a day (today by default) and an optional note. |
| Dashboard placement | Personal tasks appear in the **All** tab only, not in the per-university tabs. Each one shows "Personal" in place of course and kind. |
| Personal task status | Pending, overdue (its day has passed and it is not done), or done. In time grouping they fall into the same groups by their day (Today, This week…), and done ones move to Completed. |
| Editing | Unlike platform items, personal tasks can be renamed, moved to another day and deleted, from the details panel. |
| Dashboard without universities or tracked courses | The two current full-page empty states stay as they are: "Add your first university" and "No courses are tracked yet". The dashboard shows no personal tasks and no "Add task" there. In that situation personal tasks are used from the Calendar, which always works. |

Mockup reference (layout B), described: the header has ‹, the month and year, › and **Today**. Below it is a 7-column grid, Monday to Sunday. Days outside the month are muted and today's cell is outlined. Each day cell shows its number and a row of small coloured dots, with a legend under the grid. Below that is a panel headed "Tuesday 10 November" listing the day's rows (done checkbox, title, "· {course} · {kind}" or "· Personal"), with an "Add a task for this day" input and an **Add** button.

## 6. Constraints

- **Architecture and rules:** follow the existing architecture and AGENTS.md.
  - `crates/core` is pure and does no I/O.
  - `crates/infra` holds the SQLite repositories and migrations.
  - `src-tauri` holds the commands and DTOs.
  - Frontend command calls go only through `src/lib/api/`.
  - No new dependencies on either side, and that includes calendar libraries.
- **UI conventions:** English, sentence-case copy. Dates use the `en-GB` helpers in `src/components/results/format.ts`. Weeks start on Monday, as in the dashboard grouping. Reuse `Modal`, `ConfirmDialog`, `ErrorMessage` and `forms/*`, and keep the current Tailwind look.
- **Errors:** every rejected command is shown inline using `toCommandError` and `ErrorMessage`.
- **Validation lives in the backend command layer**, as it does for item notes:
  - The title is trimmed and required, at most 200 characters.
  - The day is a valid `YYYY-MM-DD` date.
  - The note is trimmed; blank becomes none; at most 2000 characters, reusing `validation.rs` (`MAX_NOTE_CHARS`).
- **Logging:** never log titles or notes.
- **The day is a calendar date.** Store it as a `YYYY-MM-DD` text and never convert it to a UTC instant. Platform deadlines stay instants, displayed in local time. Today's date comes from the local clock (`todayLocalIso`).

## 7. The how: what is decided

### 7.1 Storage

- **Table:** migration `0004_personal_tasks.sql` creates `personal_tasks` with these columns:
  - `id`: integer primary key
  - `title`: text, not null
  - `date`: text, not null, `YYYY-MM-DD`
  - `note`: text, nullable
  - `done`: integer, not null, default 0
  - `created_at`, `updated_at`: RFC 3339 text
- **Index:** add one on `date`.
- **Repository:** a domain type and a repository port in `crates/core`, and a SQLite repository in `crates/infra/src/db/`.

### 7.2 Commands

- **Payload:** `get_tracker` keeps its current contract. `TrackerDto` gains `personalTasks: PersonalTaskDto[]`, carrying every personal task, so the dashboard and the calendar each make one request.
- **New commands**, each returning the updated `PersonalTaskDto` except delete:
  - `create_personal_task(input)`
  - `update_personal_task(id, input)`, where `input` is `{ title, date, note }`
  - `set_personal_task_done(id, value)`
  - `delete_personal_task(id)`
- **`PersonalTaskDto`:** `{ id, title, date, note, done }`.

### 7.3 Frontend

- **Shared entry shape.** The screens turn both kinds into one entry shape: a discriminated union such as `{ kind: 'platform', item } | { kind: 'personal', task }`, or an equivalent. Grouping (`grouping.ts`), the row, the details panel and the calendar all work on it.
- **Day boundaries.** A personal task's day maps into the time groups by comparing local dates. It is never compared as an instant.
- **Calendar route.** A new `/calendar` route and a "Calendar" sidebar item, placed after Dashboard.

## 8. Left to the builder

The following are the builder's choice:
- File and component names.
- The exact entry type.
- How the calendar grid is computed, using the same Monday-start week maths as `grouping.ts`.
- Exact styling.
- How many dots fit before "+N".
- The sidebar icon.
- The colour shades, within the palette of section 5.

## 9. Calls I made (unconfirmed by the user)

- **Hidden and removed items are not on the calendar**, as they are hidden from the dashboard by default.
- **Overflow:** a crowded day shows a few dots, then "+N".
- **Done personal tasks:** they show a faded violet dot.
- **Default selection:** the Calendar opens on the current month with today selected.
- **Details panel:** clicking a row in the day list opens the same details panel as the dashboard.
- **Personal tasks in the details panel:**
  - The panel offers rename, change day, note, mark done and delete.
  - Delete asks for confirmation with `ConfirmDialog`.
  - The platform-only parts are not shown: "Open in platform", grade, kind and hide.
- **Group by status:** pending and overdue personal tasks go into **Pending**, overdue first as for platform items. Done ones go into a visible **Done** group placed after Graded and shaped like Submitted and Graded. The collapsed "Past events" section stays platform-only.
- **"Add task" placement:** the button is shown on the All tab only, since a new personal task never appears in a university tab.
- **Platform items on the calendar:**
  - An item sits on the local calendar day of its `dueAt` instant (`new Date(dueAt)` in local time, never the UTC date).
  - Items without a due date are not shown.
  - An overdue item appears only on its due day.
- **Calendar data:** the calendar reuses the dashboard's All-tab query, `getTracker({ universityId: null, includeHidden: false, includeRemoved: false })`, with the same query key. Its cache is therefore shared and refreshed by the existing sync and item invalidations. Personal-task mutations invalidate the `['tracker']` prefix.
- **No hiding or badges:** personal tasks have no "hidden" state and no "New" or "Date changed" badges.
- **Add form defaults:** the dashboard "Add task" form defaults the day to today. The calendar box uses the selected day.
- **Long titles:** a long title is truncated in the row and the tooltip shows it in full.

## 10. Testing

- **Backend:**
  - Repository tests: create, update, done, delete and ordering by date.
  - Command validation tests: empty title, title over 200 characters, bad date, note over 2000 characters.
  - Migration: applies on an existing v0.1.0 database.
- **Frontend:**
  - Grouping with personal tasks on day boundaries: today, Sunday then Monday, yesterday is overdue.
  - Calendar grid: a month starting on a Sunday, February, today's outline.
  - The dot colour per status, and the tooltip text.
  - Adding from the calendar and from the dashboard.
  - Personal tasks appear only in the All tab.
  - The details panel edit and delete flows.
- **Acceptance:** a short manual checklist in `docs/testing.md` for the user.

## 11. As built

The feature was built as specified in sections 1 to 10, on branch `phase-3-calendar` in eight tasks. This section records where the result differs from, or adds to, what the spec says. Examples use invented data only.

### Differences from the spec

- **Two extra shared modules.** The spec forbids a second copy of the row look, and the platform and personal rows needed the same markup details. `src/features/tracker/rowStyles.ts` holds the class names both rows use, and `src/features/tracker/NoteIndicator.tsx` holds the pencil mark for "has a note". Neither changes behaviour.
- **`Modal`: Escape no longer bubbles to an outer dialog.** The delete confirmation (`ConfirmDialog`, which is a `Modal`) opens inside the personal task details. With the old behaviour Escape closed both. `Modal` now calls `stopPropagation()` on Escape, so a nested dialog closes alone and the details stay open. This changes a shared component for every modal, but only for a dialog nested inside another, which did not exist before.
- **`groupByStatus` takes `now`.** The spec only fixes `groupByTime(entries, now)`. `groupByStatus(entries, now)` also takes `now`, because a personal task is overdue only by comparing its day with the local date, so "today" must be injectable for the tests. Both derive today with `toLocalIsoDay(now)`.
- **Month navigation keeps the selected day.** Pressing ‹ or › changes the viewed month only; the selected day and its list stay as they were, even when that day is no longer in the grid. **Today** is the one control that moves both.
- **Clicking a day outside the viewed month selects it** (the list below follows) **without changing the viewed month.** The spec only says clicking a day selects it.
- **Whitespace-only titles are validated by the backend.** The title field has the HTML `required` attribute, which accepts spaces, so "Enter a title" for a title of spaces comes from `validation::personal_task` and is shown inline by `ErrorMessage` with the form (or the day box) keeping its text. The day box on the calendar has no `required` at all, so an empty title takes the same path.
- **The calendar's per-day order** is `compareEntries` (a personal task counts as local midnight of its day, so it precedes that day's deadlines), which the spec leaves to the builder.
- **Personal tasks keep a violet dot when overdue.** The spec's colour list gives red to "overdue" items; as built only platform items turn red, and an overdue personal task shows the **Overdue** pill in its row but a violet dot.

### Confirmed as specified

- Storage: migration `0004_personal_tasks.sql`, table `personal_tasks`, index on `date`; `PersonalTaskRepository` port in `crates/core`, SQLite implementation in `crates/infra/src/db/personal.rs`; nothing writes `lms_items`.
- Commands `create_personal_task`, `update_personal_task`, `set_personal_task_done`, `delete_personal_task`; `TrackerDto.personalTasks` carries every personal task for every query.
- Validation order in the command layer: title (required, at most 200 characters), day (`%Y-%m-%d`), note (`item_note`, at most 2000).
- Routes and navigation: `/calendar` and a **Calendar** item after Dashboard.
- Calendar data: the dashboard's All-tab query with the same key; personal task mutations invalidate the `['tracker']` prefix.
- Documentation: `docs/frontend/tracker-universities.md`, `docs/user-guide.md`, `docs/app/tauri-commands.md`, `docs/infra/database.md`, `docs/frontend/overview.md`, and the manual checklist "Calendar and personal tasks acceptance checklist" in `docs/testing.md`.
