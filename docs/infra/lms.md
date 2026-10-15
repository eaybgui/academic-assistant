# LMS platforms (`crates/infra/src/lms`, `crates/core/src/lms`, `src-tauri/src/lms_auth`)

Phase 2 reads deliverables, exams and course events from the user's learning platforms. This
document covers the platform clients, the credential capture and the sync. The screens that use
them are in [../frontend/tracker-universities.md](../frontend/tracker-universities.md); the
commands and events are in [../app/tauri-commands.md](../app/tauri-commands.md); the tables are in
[database.md](database.md). Everything here is read-only against the platform and uses no AI.

Examples in this document are invented (`Universidad Demo`, `campus.demo.example`, course
`Álgebra Lineal`, ids such as `_10_1`).

## Where the code lives

| Area | Location | Responsibility |
|---|---|---|
| Domain and ports | `crates/core/src/domain/lms.rs`, `crates/core/src/ports/lms.rs` | `LmsUniversity`, `LmsCourse`, `LmsItem`, `ItemKind`, `PlatformStatus`, `Grade`, `ItemChange`, `EffectiveStatus`, `SyncWindow`; the `LmsClient`, `LmsClientFactory`, `LmsAuthenticator` and `LmsSyncObserver` traits and `LmsError` |
| Pure logic | `crates/core/src/lms/` | `normalize_base_url`, `ensure_session` / `adopt_credential`, `diff_items`, `LmsSyncRunner` |
| Platform clients | `crates/infra/src/lms/{moodle,blackboard,probe,factory}.rs` | `MoodleClient`, `BlackboardClient`, the public probe and `DefaultLmsClientFactory` |
| Sign-in windows | `src-tauri/src/lms_auth/` | Login window, credential capture and `TauriLmsAuthenticator` (silent renewal) |
| Persistence | `crates/infra/src/db/{universities,courses,items}.rs` | `UniversityRepository`, `CourseRepository`, `LmsItemRepository` |

The dependency rule is unchanged: `src-tauri -> infra -> core`. `core` has no I/O; it only sees
the `LmsClient` and repository traits.

## Platform URL and probe

`normalize_base_url` (core) reduces what the user types to an `https` origin: trim, prepend
`https://` when there is no scheme, reject any other scheme ("The platform URL must start with
https://"), keep host and port, lowercase it and drop path, query and fragment. Hosts need a dot;
`user@host` is rejected.

`probe` (infra) checks that the URL hosts the chosen platform before anything is stored. It makes
unauthenticated requests only:

- Moodle: `POST {base}/lib/ajax/service-nologin.php?info=tool_mobile_get_public_config` must
  report `enablewebservices` and `enablemobilewebservice` as enabled. Errors: "This URL is not a
  Moodle site", "Mobile web services are disabled on this Moodle site".
- Blackboard: `GET {base}/learn/api/public/v1/system/version` must return a `learn` object.
  Error: "This URL is not a Blackboard Learn site".

A mismatch becomes a `validation` command error; a network failure becomes `connection`.

## Credentials

The app never captures or stores the university password. The stored credential is scoped:

| Platform | Credential | Secret key |
|---|---|---|
| Moodle | The mobile web-service token (private token discarded) | `academic-assistant/university/{id}/moodle-token` |
| Blackboard | The session cookies of the platform host, as a JSON array of `{ name, value }` | `academic-assistant/university/{id}/blackboard-session` |

Both live only in Windows Credential Manager and in memory. In Credential Manager each key is a
generic credential whose target is `{key}.academic-assistant` (the `keyring` crate's
`{user}.{service}` format, with the key as user and `academic-assistant` as service). A stored
secret is at most 2560 bytes ([secure-storage.md](secure-storage.md)); a Blackboard cookie set
that would exceed it is rejected with "The Blackboard session is too large to store securely" and
only the cookie names (never the values) are logged. `SessionCookie` has a hand-written `Debug`
that prints `<redacted>` for the value.

A missing secret, or a stored Blackboard session that cannot be parsed, is reported by the factory
as `LmsError::Auth`, so it follows the same path as an expired session (silent renewal, then a
visible sign-in).

## Sign-in (`src-tauri/src/lms_auth`)

`capture_credential(app, university, mode)` opens a webview window labelled `lms-login-{id}` with
the title "Sign in to {name}" at the platform's own sign-in page and waits for the credential.

- Profile: a persistent data directory `{app_data}/lms-auth`, shared by all universities and
  separate from the main window, so the identity provider's cookies survive restarts and a new
  sign-in may finish without typing anything. Signing out does not clear it.
- The window has no IPC access (the capability file targets only the `main` window). Navigation is
  not restricted, because SSO redirects across several hosts.
- One window per university: a second interactive sign-in focuses the existing window and fails
  with "A sign-in window for this university is already open". Different universities can have
  windows open at the same time.
- Interactive mode: visible window, 10 minute limit ("Sign-in timed out"). Closing it early gives
  "Sign-in cancelled". Silent mode (renewal): hidden window, 20 second limit, then "Sign-in
  required".
- The window is destroyed on every exit path, and all login windows are destroyed when the main
  window closes.
- URLs seen in the window are never logged (the Moodle redirect contains the token and SSO URLs
  carry assertions); only university id, mode and stage are.

**Moodle.** The app generates a random 32-character passport and opens
`{base}/admin/tool/mobile/launch.php?service=moodle_mobile_app&passport=<passport>&urlscheme=moodlemobile`.
The navigation handler cancels any navigation to `moodlemobile://token=<payload>` and decodes the
payload with `decode_launch_token`: base64, then `signature:::token[:::privatetoken]`; the
signature must equal `md5(base_url + passport)`, otherwise "The Moodle sign-in could not be
verified". Only the token is kept.

**Blackboard.** The window opens `{base}/ultra`. Sign-in is detected when the window shows an
Ultra page below `/ultra/` (for example `/ultra/stream`); `/ultra` itself redirects to the login
page. Ultra is a single-page app that can change its URL without a page-load event, so the URL is
also polled every 500 ms. The cookies are then read off the main thread (WebView2 deadlocks when
they are read from the event loop thread), including HttpOnly ones, and filtered to the platform
host.

### Verification

Every captured credential is verified before it is stored (`adopt_credential`): a client is built
with the credential, `verify()` is called, and only on success is the secret written.
`complete_sign_in` also checks that the university the capture started with still exists with the
same platform and URL; if it was removed or replaced during the (possibly minutes-long) sign-in,
nothing is stored. A credential the platform rejects sets the login state to `error`; transient
connection and storage failures leave the state alone.

### Session check and renewal

`ensure_session` (used by "Check connection" and at the start of every sync) behaves as follows:

1. State `never` -> `LoginRequired` (no network call).
2. `verify()` with the stored credential. Success -> state `ok` with the platform's display name.
3. On `Auth`, one silent renewal (hidden window). Then `verify()` again.
4. Still `Auth` -> state `expired` (the previous account label is kept) and `LoginRequired`
   ("{name} needs you to sign in again").

Connection and protocol errors leave the state unchanged. Moodle reports an expired or revoked
token as `invalidtoken`, or as `accessexception` from `core_webservice_get_site_info`; both are
treated as an expired session.

## Moodle (`MoodleClient`)

Requests are `POST {base}/webservice/rest/server.php` with the form fields `wstoken`,
`wsfunction`, `moodlewsrestformat=json` and the function's arguments, sequential, with a 30 second
timeout. A body with `errorcode` is an error: `invalidtoken` -> `Auth`;
`servicenotavailable` / `accessexception` -> `Protocol` naming the function; other codes ->
`Protocol`. HTTP failures and unreadable JSON are `Protocol` ("<function> returned HTTP 500"),
network failures `Connection`. No message contains the token.

| Need | Function(s) | Mapping |
|---|---|---|
| Verify | `core_webservice_get_site_info` | `userid`, `fullname`. Every function in the table below must be in the site's `functions` list, otherwise `Protocol` ("<function> is not enabled for the Moodle mobile service on this site") |
| Courses | `core_enrol_get_users_courses(userid)` | `id` -> `external_id`; `fullname`; `shortname`; url `{base}/course/view.php?id={id}` |
| Assignments | `mod_assign_get_assignments`, then `mod_assign_get_submission_status` per assignment | `assign:{id}`, kind `assignment`, `duedate` (0 = none), url `{base}/mod/assign/view.php?id={cmid}` |
| Quizzes | `mod_quiz_get_quizzes_by_courses`, `mod_quiz_get_user_attempts` (finished), `mod_quiz_get_user_best_grade` | `quiz:{id}`, kind `quiz`, `timeclose` (0 = none), url `{base}/mod/quiz/view.php?id={coursemodule}` |
| Course events | `core_calendar_get_calendar_events` with the sync window, `userevents=0`, `siteevents=0` | `event:{id}`, kind `event`, `timestart`; only `eventtype` `course` or `group` with an empty `modulename` (assignments and quizzes already appear as items), status `not_applicable`, url `{base}/calendar/view.php?view=day&time={timestart}` |

Status and grade rules:

- **Assignment.** A grade counts only when the feedback display text is real (not empty and not
  `-`) and the numeric grade is not negative: Moodle returns a placeholder feedback with a
  negative grade (`-1`) for unmarked work, so `-1` means "no grade". HTML in the display text is
  removed and `8,50` style decimals are parsed. Otherwise: `nosubmissions = 1` ->
  `not_applicable`; a submission (or team submission) with status `submitted` -> `submitted`;
  else `not_submitted`.
- **Quiz.** A best grade (`hasgrade`) -> `graded` with `display` `"7.5 / 10"`, `score` and `max`;
  otherwise any finished attempt -> `submitted`; else `not_submitted`.
- A zero or negative timestamp is "no date".

## Blackboard (`BlackboardClient`)

Requests are `GET {base}/learn/api/...` with the stored cookies in a `Cookie` header, a 30 second
timeout, and `paging.nextPage` followed (links must start with `/learn/api/`; at most 50 pages).

Session handling: 401 or 403, or a redirect that lands outside `/learn/api/` (the login page), is
`Auth` ("Your Blackboard session has expired"). 404 is "not found". Other HTTP failures are
`Protocol`.

| Need | Endpoint | Mapping |
|---|---|---|
| Verify | `/public/v1/users/me` | `id`; display name from `name.given` + `name.family`, falling back to `userName`, then the id |
| Courses | `/public/v1/users/me/courses?expand=course&limit=100` | `courseId` -> `external_id` (e.g. `_10_1`); `course.name`; `course.courseId` -> short name; url `{base}/ultra/courses/{courseId}/outline` |
| Columns | `/public/v2/courses/{id}/gradebook/columns` | `column:{id}`, `grading.due` -> due date, url the course outline |
| Grade | `/public/v1/courses/{id}/gradebook/users/{me}` | A score for the column -> `graded`, `display` `"7.5 / 10"` using `score.possible` |
| Attempts | `/public/v1/courses/{id}/gradebook/columns/{col}/attempts?userId={me}` | An attempt whose status is not `InProgress`, `NotAttempted`, `Abandoned` or `Canceled` -> `submitted` |
| Course events | `/public/v1/calendars/items?type=Course&since=&until=` | `calendar:{id}`, kind `event`, `start`; only items whose `calendarId` is a tracked course; url `{base}/ultra/calendar` |

Column rules, in order:

1. Columns whose `scoreProviderHandle` is `resource/x-bb-attendance` are skipped (not
   deliverables), as are `Calculated` columns (totals).
2. `resource/x-bb-assignment` and `resource/x-bb-assessment` -> kind `assignment` (Ultra uses
   `x-bb-assessment` for both assignments and tests); a handle containing `test` or `asmt` ->
   `test`; any other handle -> `other`.
3. Status: a score -> `graded`; no `contentId` (a manual column with no submission flow) ->
   `not_applicable`; otherwise the attempts decide `submitted` or `not_submitted`.

Endpoints scoped to a course's gradebook treat 404 and 403 on the first page as "nothing to list":
a student may have no gradebook in a course (a 403 there is not an expired session). Anywhere
else, 403 is still an expired session, and a 403 on a later page is an error.

**Calendar chunks.** The calendar endpoint rejects ranges longer than 16 weeks, measured in the
server's local time. A 112-day chunk that crosses a daylight-saving change becomes 16 weeks plus
an hour there, so the sync window is split into consecutive chunks of at most 105 days. Items that
appear in two chunks are de-duplicated by `external_id`, keeping the first.

## Sync window and the `LmsClient::fetch_items` contract

`SyncWindow::around(now)` is `now - 180 days` to `now + 365 days`. It only limits course events;
assignments, quizzes and gradebook columns are fetched whatever their date. `fetch_items` is
all-or-nothing: any failed request fails the whole call, except the explicit "empty if missing"
Blackboard cases above.

## Sync (`LmsSyncRunner`)

`prepare(university_id)` takes a single-run lock (independent of the email `CheckRunner`) and
selects the target universities: the given one or all, excluding those whose login state is
`never`. Errors: `AlreadyRunning` ("A sync is already running"), `NothingToSync` ("Sign in to a
university first"). `execute` then syncs the universities one after another and notifies the
observer exactly once; a failure in one never stops the others.

Per university:

1. Stage `courses`: `ensure_session` (verify, silent renewal), then `list_courses` and
   `upsert_remote_courses`. New courses are stored untracked and unreviewed; courses the platform
   no longer returns become unavailable; returned ones become available again.
2. Stage `items`: `fetch_items` for the courses that are tracked and available (skipped when there
   are none).
3. Stage `saving`: `diff_items` against the stored items, then `apply_sync` in one transaction
   (inserts, updates, removals and the `items_synced_at` of the synced courses).
4. The outcome is recorded on the university (`last_sync_at`, `last_sync_outcome`,
   `last_sync_error`) and the final stage (`done`, `failed` or `login_required`) is reported.

Outcomes: `success`; `login_required` (session check said sign-in is needed, or the platform
rejected the credential during the sync, which also sets the state to `expired`); `failed`
(connection or protocol error, or storage). When a university fails nothing from it is written.

### Diff rules (`diff_items`)

The key is `(university, external_id)`. Only courses fetched in this sync are considered.

| Case | Result |
|---|---|
| Remote item not stored | Insert. Badge `new`, unless the course's `items_synced_at` is empty (its first sync) |
| Stored, due date differs | Update; badge `due_changed` remembering the original date. Moving back to the original date clears the badge. A `new` badge is kept. A date appearing or disappearing also counts as a change |
| Stored, other platform fields differ | Update silently (title, kind, url, status, grade, course) |
| Stored, not returned, course fetched | Set `removed_at` (if not already set) |
| Stored with `removed_at`, returned again | Clear `removed_at` |
| Event stored outside the sync window | Never removed: the sync could not have fetched it |
| Personal fields (`done`, `hidden`, `note`) | Never touched by a sync |

Items of untracked or unavailable courses are neither fetched nor marked removed. They stay in the
database, with their notes, and are excluded from tracker queries. The same item returned twice in
one fetch is stored once.

### Effective status

`effective_status(item, now)` derives what the dashboard shows, in this order:

1. A grade, or platform status `graded` -> `graded`.
2. Platform status `submitted`, or `done` -> `submitted`.
3. Past due and (kind `event` or platform status `not_applicable`) -> `past_event`. Something with
   nothing to submit is never "overdue".
4. Past due -> `overdue`.
5. Otherwise `pending` (including items without a date).

An item is completed when it is `submitted`, `graded` or `past_event`.

## Error handling summary

| Source | `LmsError` | Surfaces as |
|---|---|---|
| Missing/unreadable credential, rejected token, 401/403, login-page redirect | `Auth` | `auth` command error; university becomes `expired` during a sync |
| Network failure, timeout | `Connection` | `connection` |
| Unexpected JSON, HTTP 5xx, disabled web-service function | `Protocol` | `internal` (the message names the function or endpoint) |

## Testing

- `cargo test -p academic-infra` runs the clients against `wiremock` with invented JSON
  (`lms_moodle`, `lms_moodle_items`, `lms_blackboard`, `lms_blackboard_items`, `lms_probe`,
  `lms_factory`) and the repositories on in-memory databases (`db_lms`, `db_items`,
  `db_migrations`). `cargo test -p academic-core` covers URL normalization, session logic,
  `effective_status`, `diff_items` and the runner (`crates/core/tests/lms_sync.rs`) with fakes.
- Two item tests also read captures of real responses when a git-ignored `lms-local` folder
  exists and skip silently when it does not.
- Live tests and the capture tool are described in [../testing.md](../testing.md#live-lms-tests-and-the-capture-tool).

## Known limitations

- Only assignments, quizzes and course events are read. Other Moodle activity types (workshops,
  lessons, graded forums) and platform announcements are not.
- Sync is manual; there is no background sync or notification.
- The platform's own course events outside the sync window are kept as stored but not refreshed.
- A Blackboard column that is a total or an attendance record is intentionally ignored.
