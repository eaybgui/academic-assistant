# Phase 2 — LMS tracker: design

Status: design approved in conversation (2026-10-04), pending review of this document. Branch: `phase-2-lms-tracker` (created from `main` after merging Phase 1).
Related: `requirements.md`, `technical-specification.md`, `docs/superpowers/specs/2026-09-28-phase-1-email-analysis-design.md`.

## 1. Goal and scope

The product is re-centred on tracking deliverables, exams and important dates. The source of truth becomes the universities' learning platforms (UV: Moodle, UAH: Blackboard Learn Ultra), read through their APIs. **No AI is involved in this phase.** Email analysis (Phase 1) is kept, unchanged in its logic, as a secondary "Announcements" section.

### In scope
- Universities configured by the user: name, platform type (`moodle` | `blackboard`) and platform URL, validated against a public endpoint of the platform.
- Sign-in through the university's own SSO in an embedded login window; capture of a scoped credential (Moodle mobile web-service token, Blackboard session cookies) stored in Windows Credential Manager; validation of every sign-in; silent session renewal.
- Course discovery from the platform and course selection (all pre-selected, editable later).
- Manual sync (all universities or one) of graded items (assignments, quizzes, tests, with submission status and grade) and course calendar events.
- Change tracking since the last sync: `new` and `due date changed` badges until dismissed; removed items kept and flagged.
- Personal per-item state stored locally: done, hidden, note.
- Dashboard: tabs for the united view and one per university; agenda grouped by time with a "Group by status" toggle.
- Universities page (replaces the manual catalog). The email analyzer uses the tracked courses as its course catalog; email is disabled until at least one course is tracked.

### Out of scope (future phases, see §17)
Google Calendar export, background sync and notifications, platform announcements, Moodle activity types other than assignments and quizzes, a month calendar view.

### Verified facts (2026-10-04)
- **UV Moodle** (`https://aulavirtual.uv.es`): `tool_mobile_get_public_config` reports `enablewebservices=1`, `enablemobilewebservice=1`, `typeoflogin=2` (browser SSO, SAML2 IdP `as.uv.es`), `launchurl=/admin/tool/mobile/launch.php`. `login/token.php` rejects UV accounts. With a logged-in session, `core_course_get_enrolled_courses_by_timeline_classification` and `core_calendar_get_action_events_by_timesort` succeed; `mod_assign_*` / `mod_quiz_*` are not AJAX-enabled (`servicenotavailable`) and are expected to work through the mobile token.
- **UAH Blackboard** (`https://uah.blackboard.com`, Learn SaaS 4001.0): SSO via Microsoft Entra ID (SAML). With the browser session cookies, `/learn/api/public/v1/users/me`, `/v1/users/me/courses?expand=course`, `/v1/calendars/items`, `/v2/courses/{id}/gradebook/columns`, `/v1/courses/{id}/contents`, `/v1/courses/{id}/gradebook/users/{userId}` and `/v2/courses/{id}/gradebook/columns/{col}/attempts?userId=` all return 200. The Ally bearer token seen in the browser is not a Learn credential.

## 2. Decisions

| Topic | Decision |
|---|---|
| Project | Re-scope this repository (no new project). |
| Dashboard data | Only LMS API data. |
| Important dates | Graded items + course calendar events. No personal calendar entries. |
| Course selection | During setup, all pre-selected; editable later. New courses found by later syncs are added untracked and surfaced for review. |
| Change tracking | `new` and `due_changed` badges until dismissed; sync summary counts. No per-item history. |
| Personal state | `done`, `hidden`, `note`; local only; never overwritten by sync. |
| Layout | Agenda grouped by time + "Group by: Time \| Status" toggle. No month view. |
| Sign-in | Embedded login window, backend makes the API calls (approach 1). Passwords are never captured or stored. Silent renewal through a persistent login profile. |
| Manual catalog | Removed. Tracked courses feed the email analyzer. |
| Email | Secondary "Announcements" section, disabled until at least one course is tracked. |
| Google Calendar | Future phase (Google Calendar API, dedicated calendar). Phase 2 guarantees stable item ids. |

## 3. Repository layout (changes)

```text
crates/core/src/
  domain/lms.rs                 University (extended), LmsPlatform, LoginState, Course (extended),
                                LmsItem, ItemKind, PlatformStatus, Grade, ItemChange, EffectiveStatus
  ports/lms.rs                  LmsClient, LmsClientFactory, LmsAuthenticator, LmsError, Remote* types
  lms/                          LmsSyncRunner, diff, effective status, URL normalization
crates/infra/src/
  lms/moodle.rs                 MoodleClient (+ launch token decoding/signature check)
  lms/blackboard.rs             BlackboardClient
  lms/factory.rs                DefaultLmsClientFactory
  lms/probe.rs                  public-endpoint platform probe
  db/universities.rs, db/courses.rs, db/lms_items.rs
crates/infra/migrations/0002_lms.sql
src-tauri/src/
  lms_auth/                     login windows, Moodle/Blackboard capture, silent renewal (LmsAuthenticator impl)
  commands/universities.rs, commands/lms.rs, dto/lms.rs
src/features/
  tracker/                      new Dashboard (agenda, grouping, item row, details panel, banners)
  universities/                 Universities page + Add university wizard + course selection
  announcements/                former email dashboard/results, moved and renamed
```

Removed: `src/features/catalog/`, `src-tauri/src/commands/catalog.rs`, `src-tauri/src/dto/catalog.rs`, the manual-catalog parts of `CatalogRepository`, and the email `DashboardPage` (its counters move to the Announcements header). The dependency rule `src-tauri → infra → core` is unchanged.

## 4. Core contracts (`crates/core`)

### 4.1 Domain

```rust
pub enum LmsPlatform { Moodle, Blackboard }                       // "moodle" | "blackboard"
pub enum LoginState { Never, Ok, Expired, Error }                 // "never" | "ok" | "expired" | "error"
pub enum SyncOutcome { Success, Failed, LoginRequired }           // "success" | "failed" | "login_required"

pub struct LmsUniversity {
    pub id: i64, pub name: String,
    pub platform: LmsPlatform,
    pub base_url: String,                 // normalized https origin, no trailing slash
    pub login_state: LoginState,
    pub account_label: Option<String>,    // "Connected as …" (from verify)
    pub last_sync_at: Option<DateTime<Utc>>,
    pub last_sync_outcome: Option<SyncOutcome>,
    pub last_sync_error: Option<String>,  // sanitized, user-facing
    pub created_at: DateTime<Utc>,
}

pub struct LmsCourse {
    pub id: i64, pub university_id: i64,
    pub external_id: String,              // Moodle "1234" · Blackboard "_10_1"
    pub name: String, pub short_name: Option<String>, pub url: Option<String>,
    pub tracked: bool,                    // default false on insert
    pub available: bool,                  // false when the platform stops returning it
    pub reviewed: bool,                   // false until the user saves a course selection that includes it
    pub items_synced_at: Option<DateTime<Utc>>,  // None → next item sync is this course's first
}

pub enum ItemKind { Assignment, Quiz, Test, Event, Other }
pub enum PlatformStatus { NotSubmitted, Submitted, Graded, NotApplicable }
pub struct Grade { pub display: String, pub score: Option<f64>, pub max: Option<f64> }
pub enum ItemChange { New, DueChanged { previous_due_at: Option<DateTime<Utc>> } }

pub struct LmsItem {
    pub id: i64, pub university_id: i64, pub course_id: i64,
    pub external_id: String,              // stable, unique per university: "assign:812", "quiz:77",
                                          // "event:9031", "column:_20_1", "calendar:<id>"
    pub kind: ItemKind, pub title: String,
    pub due_at: Option<DateTime<Utc>>, pub url: Option<String>,
    pub platform_status: PlatformStatus, pub grade: Option<Grade>,
    pub change: Option<ItemChange>,
    pub removed_at: Option<DateTime<Utc>>,
    pub done: bool, pub hidden: bool, pub note: Option<String>,   // personal; max note length 2000 chars
    pub first_seen_at: DateTime<Utc>, pub updated_at: DateTime<Utc>,
}

pub enum EffectiveStatus { Pending, Overdue, Submitted, Graded, PastEvent }
pub fn effective_status(item: &LmsItem, now: DateTime<Utc>) -> EffectiveStatus;
```

`effective_status` rules, in order: `grade.is_some()` or `platform_status == Graded` → `Graded`; `platform_status == Submitted` or `done` → `Submitted`; `kind == Event` and `due_at < now` → `PastEvent`; `due_at < now` → `Overdue`; otherwise `Pending`. An item is **completed** when its status is `Submitted`, `Graded` or `PastEvent`.

Serialized enum values (DB and DTOs) are snake_case. `ItemChange` is stored as two columns (`change_kind`: `new` | `due_changed` | NULL, `previous_due_at`).

Secret keys (new helpers next to the existing ones): `academic-assistant/university/{id}/moodle-token`, `academic-assistant/university/{id}/blackboard-session` (JSON array of `{ name, value }` cookies for the platform host).

### 4.2 Ports

```rust
pub struct LmsIdentity { pub user_id: String, pub display_name: String }
pub struct RemoteCourse { pub external_id: String, pub name: String,
                          pub short_name: Option<String>, pub url: Option<String> }
pub struct RemoteCourseRef { pub external_id: String }
pub struct SyncWindow { pub from: DateTime<Utc>, pub to: DateTime<Utc> }   // now − 180 d … now + 365 d
pub struct RemoteItem {
    pub course_external_id: String, pub external_id: String, pub kind: ItemKind,
    pub title: String, pub due_at: Option<DateTime<Utc>>, pub url: Option<String>,
    pub platform_status: PlatformStatus, pub grade: Option<Grade>,
}

#[derive(thiserror::Error)]
pub enum LmsError { Auth(String), Connection(String), Protocol(String) }   // kind() → ErrorKind

#[async_trait]
pub trait LmsClient: Send + Sync {
    async fn verify(&self) -> Result<LmsIdentity, LmsError>;
    async fn list_courses(&self) -> Result<Vec<RemoteCourse>, LmsError>;
    /// All or nothing: any failed request fails the whole call.
    async fn fetch_items(&self, courses: &[RemoteCourseRef], window: &SyncWindow)
        -> Result<Vec<RemoteItem>, LmsError>;
}
pub trait LmsClientFactory: Send + Sync {
    /// Reads the credential from SecretStore; missing credential → LmsError::Auth.
    fn create(&self, university: &LmsUniversity) -> Result<Box<dyn LmsClient>, LmsError>;
}
#[async_trait]
pub trait LmsAuthenticator: Send + Sync {
    /// Hidden login attempt (≤ 20 s). On success the new credential is already stored
    /// and verified. Err(LmsError::Auth) when user interaction is required.
    async fn renew_silently(&self, university: &LmsUniversity) -> Result<(), LmsError>;
}
```

Repositories (synchronous, implemented by `Database`):
- `UniversityRepository`: `list`, `get`, `insert(name, platform, base_url, now)`, `rename`, `delete` (cascades courses and items), `set_login(id, state, account_label)`, `record_sync(id, at, outcome, error)`.
- `CourseRepository`: `list(university_id)`, `upsert_remote(university_id, &[RemoteCourse])` (insert new as untracked and unreviewed, update name/short name/url, mark missing as unavailable, re-mark returned as available), `set_tracked(university_id, &[course_id])` (exactly these become tracked; every course of the university becomes reviewed), `tracked_count()`.
- `LmsItemRepository`: `list_for_university(id)` (all stored items, for diffing), `apply_sync(university_id, &SyncChanges, now)` (one transaction: inserts, updates, removals, `items_synced_at` of the synced courses), `query(&TrackerQuery)`, `get`, `set_done`, `set_hidden`, `set_note`, `dismiss_change(id)`, `dismiss_all(university_id: Option<i64>)`.
- `CatalogRepository` keeps only `catalog()`, now returning the tracked, available courses (`aliases = [short_name]` when present and different from the name) and the universities. All manual CRUD methods are removed.

## 5. Platform connectors (`crates/infra/src/lms`)

Both use the shared `reqwest::Client`, 30 s timeout, sequential requests, and map: HTTP 401/403 or Moodle `invalidtoken` → `Auth`; network/timeout → `Connection`; unexpected JSON, HTTP 5xx, or a Moodle access exception on a function → `Protocol` (message names the function/endpoint, never the token or cookies).

### 5.1 MoodleClient
`POST {base}/webservice/rest/server.php` with form fields `wstoken`, `wsfunction`, `moodlewsrestformat=json` and the function arguments. A JSON body with `exception`/`errorcode` is an error.

| Need | Function | Mapping |
|---|---|---|
| verify | `core_webservice_get_site_info` | `userid`, `fullname`. Every function below must be listed in `functions`, else `Protocol("<function> is not enabled for the mobile service")` |
| courses | `core_enrol_get_users_courses(userid)` | `id` → `external_id`, `fullname`, `shortname`, url `{base}/course/view.php?id={id}` |
| assignments | `mod_assign_get_assignments(courseids)`, then `mod_assign_get_submission_status(assignid)` per assignment | `external_id = "assign:{id}"`, `duedate` (0 → None), url `{base}/mod/assign/view.php?id={cmid}`; status: `lastattempt.submission.status == "submitted"` → `Submitted`, else `NotSubmitted`; `feedback.gradefordisplay` present → `Graded` with that display text |
| quizzes | `mod_quiz_get_quizzes_by_courses(courseids)`, `mod_quiz_get_user_attempts(quizid, status="finished")`, `mod_quiz_get_user_best_grade(quizid)` | `external_id = "quiz:{id}"`, kind `Quiz`, `timeclose` (0 → None), url `{base}/mod/quiz/view.php?id={coursemodule}`; any finished attempt → `Submitted`; `hasgrade` → `Graded`, `score = grade`, `max = quiz.grade` |
| course events | `core_calendar_get_calendar_events(events.courseids, options.timestart/timeend from SyncWindow, userevents=0, siteevents=0)` | keep `eventtype` in {`course`, `group`} and empty `modulename`; `external_id = "event:{id}"`, kind `Event`, `due_at = timestart`, `NotApplicable` |

Launch token decoding (pure function in `lms/moodle.rs`, unit-tested): input is the part after `moodlemobile://token=`; base64-decode; split on `:::`; require ≥ 2 parts; require `parts[0] == hex(md5(base_url + passport))`; return `parts[1]` only (the private token is discarded). Adds the `md-5` crate to `infra`.

### 5.2 BlackboardClient
`GET {base}/learn/api/public/...` with the stored cookies in a `Cookie` header; follow `paging.nextPage` until absent.

| Need | Endpoint | Mapping |
|---|---|---|
| verify | `/v1/users/me` | `id`, display name from `name.given` + `name.family` (fallback `userName`) |
| courses | `/v1/users/me/courses?expand=course&limit=100` | `courseId` → `external_id`, `course.name`, `course.courseId` → short name, url `{base}/ultra/courses/{courseId}/outline` |
| graded items | `/v2/courses/{id}/gradebook/columns` | skip calculated/total columns (`grading.type == "Calculated"`); kind from `scoreProviderHandle` (assignment handle → `Assignment`, test/assessment handle → `Test`, otherwise `Other`; exact handle strings taken from the M1 fixtures); `external_id = "column:{id}"`, `due_at = grading.due` |
| status and grade | `/v2/courses/{id}/gradebook/columns/{col}/attempts?userId={me}` and `/v1/courses/{id}/gradebook/users/{me}` | any attempt whose status is not `InProgress` → `Submitted`; a released score (from the user grade entry, or the attempt's `displayGrade` when `readyToPost`) → `Graded` with `display` from `displayGrade`/score and `max` from the column's `score.possible` |
| course events | `/v1/calendars/items?type=Course&since=&until=` | split the SyncWindow into ≤ 16-week chunks; keep items whose `calendarId` is a tracked course id; `external_id = "calendar:{id}"`, kind `Event`, `due_at = start` |

### 5.3 Probe (`lms_probe`)
Public, unauthenticated check run before saving a university:
- URL normalization (core, pure): trim; prepend `https://` if no scheme; reject non-`https`; keep scheme + host (+ port); drop path, query and fragment.
- Moodle: `POST {base}/lib/ajax/service-nologin.php?info=tool_mobile_get_public_config` must return `enablewebservices = 1` and `enablemobilewebservice = 1`.
- Blackboard: `GET {base}/learn/api/public/v1/system/version` must return a `learn` object.
- Failures → `validation` with a precise message ("This URL is not a Moodle site", "Mobile web services are disabled on this Moodle site", …).

## 6. Sign-in, session check and silent renewal (`src-tauri/src/lms_auth`)

### 6.1 Login window
- `WebviewWindowBuilder` with label `lms-login-{university_id}`, external URL, title "Sign in to {name}", data directory `{app_data}/lms-auth` (persistent, separate from the main window; shared by all universities so SSO cookies survive restarts).
- The capability file keeps targeting only `main`: the login window has no IPC access. Navigation is not restricted (SSO redirects across the platform, `login.microsoftonline.com`, `as.uv.es`).
- One login window per university at a time; a second request focuses the existing one.
- Closing the window before capture → `CommandError { kind: "auth", message: "Sign-in cancelled" }`.

### 6.2 Moodle capture
1. Generate a random `passport` (≥ 16 alphanumeric chars).
2. Open `{base}/admin/tool/mobile/launch.php?service=moodle_mobile_app&passport={passport}&urlscheme=moodlemobile`.
3. Navigation handler: on any URL starting with `moodlemobile://token=`, cancel the navigation, decode with §5.1's function, close the window.

### 6.3 Blackboard capture
1. Open `{base}/ultra`.
2. On page load finished, if the URL starts with `{base}/ultra/` (and is not under `/webapps/login`), read the window's cookies for `{base}` (including HttpOnly), keep those whose domain matches the platform host, close the window.

### 6.4 Validation (every sign-in)
After capture: store the credential in a temporary in-memory value, build the client, call `verify()`. Success → `SecretStore::set`, `UniversityRepository::set_login(Ok, display_name)`. Failure → discard the credential, `set_login(Error, None)`, return the error. The frontend receives only the resulting `UniversityDto`.

### 6.5 Silent renewal (`LmsAuthenticator` implementation)
Same flow as §6.2/§6.3 with the window created hidden. Success within 20 s → credential stored (after `verify`), `Ok(())`. Timeout or any page that needs input → close the hidden window, `Err(LmsError::Auth("Sign-in required"))`.

### 6.6 Sign-out
`lms_sign_out` deletes the secret and sets `LoginState::Never`. The `lms-auth` profile is not cleared (SSO cookies remain, so the next sign-in may complete without typing).

### 6.7 Logging
Never log URLs observed in the login window (the `moodlemobile://` URL contains the token; SSO URLs carry assertions). Log only university id, stage (`opened`, `captured`, `verified`, `cancelled`, `timeout`, `failed`) and error kind.

## 7. Sync (`core::lms::LmsSyncRunner`)

`LmsSyncRunner::start(university_id: Option<i64>) -> Result<(), LmsSyncError>`:
1. Preconditions (no state change): another LMS sync in progress → `AlreadyRunning`; no university with `login_state != Never` in the target set → `NothingToSync`. Uses its own single-run lock (same pattern as `pipeline/lock.rs`); independent from the email `CheckRunner`.
2. Spawn; for each target university, sequentially:
   1. `factory.create(university)`; `list_courses()`; `CourseRepository::upsert_remote`.
   2. `fetch_items(tracked ∧ available courses, SyncWindow)`.
   3. On `LmsError::Auth` in 1–2: `authenticator.renew_silently`; on success retry steps 1–2 once; otherwise `set_login(Expired)`, outcome `LoginRequired`.
   4. Diff (pure function) against `list_for_university`, then `apply_sync` in one transaction.
   5. `record_sync(outcome, error)`. `Connection`/`Protocol` errors → outcome `Failed`; nothing from that university is written. Other universities continue.
3. Emit `lms-sync:progress` at each stage and `lms-sync:finished` once with all results.

Diff rules (key: `(university_id, external_id)`; only courses fetched in this sync are considered):

| Case | Result |
|---|---|
| Remote item not stored | Insert. `change = New`, unless the course's `items_synced_at` is None (first sync of the course) → no badge. |
| Stored, `due_at` differs | Update; `change = DueChanged { previous }` where `previous` is the existing `DueChanged.previous_due_at` if present, otherwise the stored `due_at`. If the new `due_at` equals that `previous`, clear the change. An item with `change = New` keeps `New`. |
| Stored, other fields differ | Update silently (title, kind, url, status, grade). |
| Stored, not returned | Set `removed_at = now` (if not already set). |
| Stored with `removed_at`, returned again | Clear `removed_at`. |
| Personal fields | Never modified by sync. |

Course-level rules: items of untracked or unavailable courses are neither fetched nor marked removed; they stay in the database and are excluded from tracker queries.

`LmsSyncError` maps to `CommandError`: `AlreadyRunning` → `conflict`, `NothingToSync` → `validation`.

## 8. SQLite (`0002_lms.sql`)

The migration runner is generalized from "apply version 1" to an ordered list of `(version, sql)` pairs, each applied in its own transaction with its `schema_migrations` row.

`0002_lms.sql`:
1. Deletes all rows of `courses` and `universities` (FK actions set `ai_results.university_id`, `ai_results.course_id` and `accounts.default_university_id` to NULL).
2. Recreates `universities` and `courses` with the new columns (needed to add NOT NULL/CHECK columns and to drop `UNIQUE (university_id, name)` on courses), keeping table names so existing foreign keys stay valid:

```sql
universities (id INTEGER PRIMARY KEY, name TEXT NOT NULL UNIQUE,
  platform TEXT NOT NULL CHECK (platform IN ('moodle','blackboard')),
  base_url TEXT NOT NULL UNIQUE,
  login_state TEXT NOT NULL DEFAULT 'never' CHECK (login_state IN ('never','ok','expired','error')),
  account_label TEXT, last_sync_at TEXT,
  last_sync_outcome TEXT CHECK (last_sync_outcome IN ('success','failed','login_required')),
  last_sync_error TEXT, created_at TEXT NOT NULL)
courses (id INTEGER PRIMARY KEY,
  university_id INTEGER NOT NULL REFERENCES universities(id) ON DELETE CASCADE,
  external_id TEXT NOT NULL, name TEXT NOT NULL, short_name TEXT, url TEXT,
  tracked INTEGER NOT NULL DEFAULT 0, available INTEGER NOT NULL DEFAULT 1,
  reviewed INTEGER NOT NULL DEFAULT 0, items_synced_at TEXT, created_at TEXT NOT NULL,
  UNIQUE (university_id, external_id))
lms_items (id INTEGER PRIMARY KEY,
  university_id INTEGER NOT NULL REFERENCES universities(id) ON DELETE CASCADE,
  course_id INTEGER NOT NULL REFERENCES courses(id) ON DELETE CASCADE,
  external_id TEXT NOT NULL,
  kind TEXT NOT NULL CHECK (kind IN ('assignment','quiz','test','event','other')),
  title TEXT NOT NULL, due_at TEXT, url TEXT,
  platform_status TEXT NOT NULL CHECK (platform_status IN ('not_submitted','submitted','graded','not_applicable')),
  grade_display TEXT, grade_score REAL, grade_max REAL,
  change_kind TEXT CHECK (change_kind IN ('new','due_changed')), previous_due_at TEXT,
  removed_at TEXT, done INTEGER NOT NULL DEFAULT 0, hidden INTEGER NOT NULL DEFAULT 0, note TEXT,
  first_seen_at TEXT NOT NULL, updated_at TEXT NOT NULL,
  UNIQUE (university_id, external_id))
CREATE INDEX idx_lms_items_due ON lms_items (university_id, due_at);
```

The exact recreate sequence (rename/copy vs. drop/create, foreign-key pragma handling inside the migration transaction) is defined in the implementation plan and covered by a migration test over a database seeded with Phase 1 data (results and accounts survive, their university/course links become NULL).

## 9. Tauri interface

### 9.1 DTOs (TypeScript notation)

```ts
type LmsPlatform = 'moodle'|'blackboard';
type LoginState = 'never'|'ok'|'expired'|'error';
type SyncOutcome = 'success'|'failed'|'login_required';
type ItemKind = 'assignment'|'quiz'|'test'|'event'|'other';
type EffectiveStatus = 'pending'|'overdue'|'submitted'|'graded'|'past_event';

interface LmsUniversityDto { id: number; name: string; platform: LmsPlatform; baseUrl: string;
  loginState: LoginState; accountLabel: string|null;
  lastSync: { at: string|null; outcome: SyncOutcome|null; error: string|null };
  courses: { tracked: number; total: number; unreviewed: number } }
interface UniversityInput { name: string; platform: LmsPlatform; baseUrl: string }
interface LmsCourseDto { id: number; universityId: number; name: string; shortName: string|null;
  url: string|null; tracked: boolean; available: boolean; isNew: boolean /* !reviewed */ }

interface GradeDto { display: string; score: number|null; max: number|null }
interface LmsItemDto { id: number; universityId: number; universityName: string;
  courseId: number; courseName: string; courseShortName: string|null;
  externalId: string; kind: ItemKind; title: string; dueAt: string|null; url: string|null;
  status: EffectiveStatus; grade: GradeDto|null;
  change: { kind: 'new' } | { kind: 'due_changed'; previousDueAt: string|null } | null;
  removedAt: string|null; done: boolean; hidden: boolean; note: string|null }
interface TrackerQuery { universityId: number|null; includeHidden: boolean; includeRemoved: boolean }
interface TrackerDto { items: LmsItemDto[]; universities: LmsUniversityDto[]; syncRunning: boolean }

interface LmsSyncResultDto { universityId: number; universityName: string; outcome: SyncOutcome;
  newItems: number; dueChanged: number; removed: number; error: string|null }
```

### 9.2 Commands

| Command | Input → Output | Notes |
|---|---|---|
| `lms_probe` | `{ url, platform } → { baseUrl }` | §5.3; `validation` on failure |
| `get_universities` | `() → UniversityDto[]` | |
| `add_university` | `UniversityInput → UniversityDto` | probes again server-side; duplicate name or URL → `conflict` |
| `rename_university` | `{ id, name } → UniversityDto` | platform and URL are immutable (delete and re-add) |
| `remove_university` | `{ id } → ()` | deletes secret, courses, items (notes included); UI confirms |
| `lms_sign_in` | `{ id } → UniversityDto` | visible login window; resolves after capture + verify, or `auth` error |
| `lms_sign_out` | `{ id } → UniversityDto` | |
| `lms_fetch_courses` | `{ id } → CourseDto[]` | `list_courses` + `upsert_remote`; requires `loginState = ok` |
| `set_tracked_courses` | `{ universityId, courseIds } → CourseDto[]` | exactly these become tracked; all courses of the university become reviewed |
| `get_courses` | `{ universityId } → CourseDto[]` | |
| `lms_sync` | `{ universityId: number\|null } → ()` | starts §7; result via events |
| `get_tracker` | `TrackerQuery → TrackerDto` | non-removed, non-hidden items of tracked courses unless the flags say otherwise; status computed with `effective_status` |
| `set_item_done` / `set_item_hidden` | `{ id, value: boolean } → LmsItemDto` | |
| `set_item_note` | `{ id, note: string\|null } → LmsItemDto` | trimmed; empty → null; > 2000 chars → `validation` |
| `dismiss_item_change` | `{ id } → LmsItemDto` | |
| `dismiss_all_changes` | `{ universityId: number\|null } → number` | rows updated |

Removed: `get_catalog`, `add_university`/`update_university`/`remove_university` (old catalog signatures), `add_course`/`update_course`/`remove_course`, `get_dashboard` (replaced by an Announcements header query reusing `ResultRepository::unseen_counts` and the last run). `check_now` gains the precondition `NoCoursesConfigured` → `validation`.

### 9.3 Events
- `lms-sync:progress` `{ universityId, universityName, stage: 'renewing'|'courses'|'items'|'saving'|'done'|'failed'|'login_required' }`
- `lms-sync:finished` `{ results: LmsSyncResultDto[] }` (exactly once per started sync)

## 10. User interface

Sidebar: **Dashboard** (`/`), **Announcements** (`/announcements`, disabled with tooltip "Add a university and track courses first" while no course is tracked; unseen badge as today), **Universities**, **Email accounts**, **Email runs**, **Settings**.

- **First run**: Dashboard shows a single card "Add your first university".
- **Add university wizard** (modal, 3 steps): (1) name, platform, URL → `lms_probe` → `add_university`; (2) "Sign in" → `lms_sign_in`, shows "Connected as {name}"; (3) `lms_fetch_courses`, list with all courses ticked (name, short name, "not visible on platform" when unavailable) → `set_tracked_courses` → `lms_sync({ universityId })` → navigate to Dashboard. Closing the wizard after step 1 leaves the university saved and resumable from the Universities page.
- **Dashboard**: tabs `All` + one per university; "Group by: Time | Status"; Sync button (all, or the selected university); "Last synced …". Tab and grouping persisted in `localStorage` (try/catch, as the sidebar preference).
  - Time grouping (local time, weeks start Monday): Overdue · Today · This week · Next week · Later · No date; completed items in a collapsed "Completed (n)" section.
  - Status grouping: Pending (overdue highlighted first) · Submitted · Graded · Past events (collapsed); each sorted by due date ascending, undated last.
  - Item row: university chip, title, course short name (or name) and kind, status pill, grade, due date/time, change badge with dismiss (×), done checkbox, note indicator. "Dismiss all" next to the sync summary.
  - Details panel (click a row): full dates, grade, "Removed from platform on …" when applicable, "Open in {platform}" (opener plugin, `https://` only), note editor (save on blur/button), hide toggle.
  - Filters: "Show hidden", "Show removed" (off by default).
  - Banners: per university with `login_required`/`expired` → "{name} needs you to sign in again" + button (`lms_sign_in`, then `lms_sync` for it); after each sync, a summary ("3 new, 1 date changed, 1 removed"; failures listed per university).
- **Universities page**: one card per university (platform, URL, "Connected as …", login state, last sync, tracked/total courses, "N new courses found" link) with Sign in/Sign out, Sync, Edit courses (same list as wizard step 3), Rename, Delete (confirmation: "Deletes this university, its courses, items and your notes.").
- **Announcements**: the Phase 1 Results page plus a header with "Check now", run progress and the unseen counters previously on the email dashboard.
- **Email accounts**: the default university selector lists configured universities.

All command calls go through `src/lib/api/` (client, types, query keys, events), with React Query invalidation on `lms-sync:finished`.

## 11. Email integration
- `CatalogRepository::catalog()` returns tracked, available courses (with `short_name` as alias) and the universities, so `EmailAnalyzer` and `CheckRunner` are unchanged apart from the new precondition.
- `CheckRunner::prepare`: `CourseRepository::tracked_count() == 0` → `RunError::NoCoursesConfigured` (before `NoEnabledAccounts`).
- Phase 1 results keep working; their university/course links are NULL after migration `0002`.

## 12. Security and logging
- LMS credentials are scoped (Moodle mobile token, Blackboard session cookies); university passwords are never captured or stored. Credentials live only in Windows Credential Manager and in memory; never in SQLite, logs, DTOs, events or error messages.
- Windows Credential Manager limits a credential blob to 2560 bytes. The Blackboard cookie set is filtered to the platform host; if it still exceeds the limit, M1 determines the minimal required cookie names and only those are stored (see §16).
- The login window has no IPC capability; the main window's CSP and capabilities are unchanged except for any permission strictly required to create the login window from Rust (none expected, since windows are created backend-side).
- LMS access is read-only: only `GET` (Blackboard) and read-only web-service functions (Moodle).
- Logs: university id, stage, counters, error kind and sanitized messages. Never tokens, cookies, login-window URLs, item titles or notes.

## 13. Testing and validation
- `core`: URL normalization; `effective_status`; diff rules (first-sync badge suppression, due change and revert, `New` kept on due change, removal and reappearance, untracked/unavailable courses, personal fields untouched); `LmsSyncRunner` with fake client/factory/authenticator/repositories (all-or-nothing, silent renewal success and failure, per-university isolation, single-run lock, events emitted once).
- `infra`: Moodle launch-token decoding and signature check; Moodle and Blackboard clients against `wiremock` with fixtures captured in M1 and anonymized (paging, chunked calendar window, error mapping, skipped calculated columns, event filtering); probe; repositories on a temp database; migration `0002` over a Phase 1 database; `#[ignore]` live tests (`verify`, `list_courses`) enabled by env var, reading the stored credential.
- `src-tauri`: commands with fakes (`test_support.rs`), including `NoCoursesConfigured`, note validation and DTO mapping.
- Frontend: API client; wizard steps; time and status grouping (week boundaries, completed section); item row actions; banners; Announcements gating.
- Gates (unchanged): `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, `npm run typecheck`, `npm run lint`, `npm test`, `npm run build`.
- Manual acceptance checklist added to `docs/testing.md` (see M1 and M5 below).

## 14. Milestones

| # | Content | Exit criteria |
|---|---|---|
| **M1 — gate** | Migration runner + `0002` (universities/courses part), `University`/`Course` domain and repositories, URL normalization, `lms_probe`, login windows and capture (§6), `verify()` for both clients, `LmsAuthenticator` with silent renewal, `add_university`/`lms_sign_in`/`lms_sign_out`/`get_universities`, minimal Universities page (add, sign in, sign out) | **Validated by the user on the desktop with real accounts**: first sign-in on UV and UAH shows "Connected as …"; restart keeps the login; a forced Blackboard session expiry renews silently (or falls back to a visible login with a clear message); an invalid Moodle token is detected; credential sizes fit Credential Manager; observed token/session lifetimes recorded; anonymized API fixtures captured. **No later milestone starts before this gate passes.** |
| M2 | `list_courses` and `fetch_items` for Moodle and Blackboard (parallelizable) | Fixture tests pass; live test lists the real courses |
| M3 | `lms_items` table, `LmsItemRepository`, diff, `LmsSyncRunner`, sync/course/item commands and events | Core/infra/command tests pass; a real sync stores items for both universities |
| M4 | Wizard course step, Dashboard (tabs, grouping, rows, details, banners), Universities page complete (parallelizable by feature) | UI tests pass; dashboard shows real data for both universities |
| M5 | Email integration (§11), Announcements move and sidebar renames, removal of catalog code, docs update (`requirements.md`, `technical-specification.md`, `README.md`, `docs/`) | All gates pass; user runs the acceptance checklist; phase branch merged into `main` |

## 15. Git workflow
`main` ← `phase-2-lms-tracker` ← feature branches (one per plan task, created from and merged back into the phase branch). The phase branch merges into `main` only after the user validates Phase 2.

## 16. Risks
- **Tauri cookie API may not return HttpOnly cookies on WebView2.** Fallback: read them through WebView2's `ICoreWebView2CookieManager` via `with_webview`. Resolved in M1.
- **WebView2 may not surface the `moodlemobile://` navigation to the navigation handler.** Fallback: register `moodlemobile` as an app URL protocol (deep link). Resolved in M1.
- **Microsoft Entra (UAH) may block embedded browsers or always require MFA.** Silent renewal then always falls back to the visible login; the product still works. Resolved in M1.
- **Credential Manager 2560-byte limit** for the Blackboard cookie set. Mitigation in §12. Resolved in M1.
- **UV may configure token expiry; Blackboard sessions are short.** Covered by `login_required` and silent renewal.
- **Blackboard calendar range limits and unknown `scoreProviderHandle` values.** Chunked window; unknown handles map to `Other`; fixtures from M1 fix the mapping.
- **Platform policies.** Only documented APIs, read-only, at user-triggered frequency, using the user's own credentials.

## 17. Future phases
- Google Calendar export through the Google Calendar API into a dedicated "Academic Assistant" calendar, upserting by `(university, external_id)`.
- Background sync and Windows notifications.
- Platform announcements (Blackboard `announcements`, Moodle news forum) feeding the Announcements section.
- Other graded Moodle activity types (workshop, lesson, graded forums).

## 18. Amendments made while planning (2026-10-04)

These override the sections above where they differ.

1. **Naming.** The LMS domain types are `LmsUniversity` and `LmsCourse`, and their DTOs `LmsUniversityDto` and `LmsCourseDto` (§4.1, §9.1), wherever the text above says `University`, `Course`, `UniversityDto` or `CourseDto` in an LMS context. The existing `University`, `Course` and `Catalog` types (and the `UniversityDto`/`CourseDto` catalog DTOs) stay as the analyzer's read-only catalog projection built by `CatalogRepository::catalog()`.
2. **`get_catalog` stays**, read-only, returning that projection (used by the email account form and the results course filter). Only the catalog CRUD commands are removed.
3. **Migrations.** `0002_lms.sql` contains only `universities` and `courses` (§8 items 1–2). `lms_items` and its index move to `0003_lms_items.sql`, delivered in M3, so a database migrated during the M1 validation never needs an edited migration.
4. **Session check.** New core function `ensure_session` (verify; on `Auth`, silent renewal, then verify again; updates `login_state`) and command `lms_check_session { id } → LmsUniversityDto`, shown as "Check connection" on the Universities page. `LmsSyncRunner` (M3) calls the same function before fetching.
5. **Incremental port.** In M1 `LmsClient` has `verify` and `list_courses`; M2 adds `fetch_items` and the item types.
6. **Fixtures.** Capturing anonymized API fixtures is the first task of M2 instead of an M1 exit criterion.
7. **Stored credential readability.** A stored Blackboard session that cannot be parsed, or a missing secret, is reported by the factory as `LmsError::Auth`, so it follows the renewal path.
