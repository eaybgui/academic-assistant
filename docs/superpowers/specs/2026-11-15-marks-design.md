# Marks section: design (short)

Date: 2026-11-15 · Status: approved by the user in chat ("go ahead, write the plan and begin implementation").

## Intent

The user wants a **Marks** section that lists every grade received for submitted work, grouped by university and then by subject, so all marks are in one place. It is for both the desktop and the Android app.

## User decisions

- **Show only what the app already fetches.** No new platform calls. Moodle (UV) gives assignment and quiz grades. Blackboard (UAH) gives every gradebook column except attendance.
- **All grades, any scale.** Numbers, letters and pass/fail are shown exactly as the platform displays them, with no scale shown separately.
- **Grouping:** by university, then by subject (course).
- **Platforms:** desktop and Android, from the shared frontend.

## Approaches

1. **Frontend only, reusing `get_tracker` (chosen).** The tracker already returns every item of the tracked courses with its `grade` (`display`, `score`, `max`). The page filters graded items and groups them. There are no backend, DTO or database changes, and it works the same on desktop and Android.
2. **A new backend command returning marks:** rejected. It duplicates `get_tracker` filtering for no gain at this size.
3. **New Moodle grade-report calls** (`gradereport_user_get_grade_items`) for complete UV marks: rejected by the user for now, possibly later.

## Design

- **Route and navigation:** a new `/marks` route and a "Marks" navigation item placed after Calendar: Dashboard, Calendar, Marks, Universities.
  - On Android phones this gives four bottom tabs; on tablets and desktop it is a sidebar entry.
  - It is not an email feature, so it is always visible.
- **Data:** `getTracker({ universityId: null, includeHidden: true, includeRemoved: false })` (my call). Hidden items still count as marks, because hiding only tidies the Dashboard. Items removed from the platform are left out, because their grade may be stale.
  - A mark is any item whose `grade` is not null.
  - Personal tasks never have grades.
- **Layout:**
  - One section per university, in the order the tracker returns them, headed by the university name.
  - Inside each, one collapsible group per subject, open by default and in the dashboard's section style. Its header shows the course name (the short name in muted text when it differs) and the number of marks.
  - Each subject sorts by course name, using the existing locale-aware collator.
  - Inside a subject, rows are sorted by due date (oldest first, undated last), then by title.
  - Each row shows the title, the kind, the due date or "No date", and the grade's `display` text emphasised. Clicking the title opens the existing item details panel.
- **Empty states:**
  - No universities: "No marks yet." plus a link to Universities.
  - Universities but no graded items: "No marks yet. Graded work appears here after a sync."
  - Universities with no marks are not shown.
- **Unchanged:** sync, grades fetching and the Dashboard and Calendar sections.

## Non-goals

No averages, totals, weighting, new-mark badges, exports, or new platform calls.

## Left to the builder

Component and file names, the marks icon, exact styles within the existing look, and test layout.
