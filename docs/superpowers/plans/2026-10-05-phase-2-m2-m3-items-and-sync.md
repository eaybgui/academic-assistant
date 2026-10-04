# Phase 2 · M2+M3 — Items and sync: Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fetch graded items and course events from Moodle and Blackboard, store them in `lms_items`, sync them with change detection (new / due date changed / removed), and expose course selection, sync and tracker commands to the frontend.

**Architecture:** `core` gains the item domain, `effective_status`, a pure `diff_items` function, the `LmsItemRepository` port and `LmsSyncRunner` (reusing the existing single-run lock and `ensure_session`). `infra` gains `fetch_items` for both clients (validated against fixtures captured from the user's real accounts), migration `0003_lms_items.sql` and the item repository. `src-tauri` gains the course, sync and tracker commands plus `lms-sync:*` events. No UI in this plan (M4).

**Tech Stack:** Rust (Tauri 2.12, rusqlite, reqwest 0.13 without `form`/`query` features, wiremock, chrono), existing workspace crates only.

**Spec:** `docs/superpowers/specs/2026-10-04-phase-2-lms-tracker-design.md` — §4, §5, §7, §8, §9 and **§18 amendments** (override earlier text). Plus the amendments in "Spec amendments made by this plan" below.

## Global Constraints

- Follow `AGENTS.md`: implement only your task, no unrelated refactors, no new dependencies, report deviations instead of improvising.
- Dependency rule `src-tauri → infra → core`; `core` stays free of I/O.
- Secrets (Moodle token, Blackboard cookies) only in `SecretStore` and in memory; never in SQLite, logs, DTOs, events, errors, fixtures or test output. Item titles and notes are personal data: never log them.
- LMS access is read-only: Moodle read functions and Blackboard `GET` only.
- `fetch_items` is all-or-nothing: any failed request fails the whole call (spec §4.2), except the explicitly listed "404 means empty" Blackboard endpoints in Task 5.
- Stable item ids (`external_id`), unique per university: `assign:{id}`, `quiz:{id}`, `event:{id}` (Moodle); `column:{id}`, `calendar:{id}` (Blackboard).
- Timestamps stored as RFC 3339 UTC text; serialized enums snake_case; DTO fields camelCase.
- `SyncWindow`: from now − 180 days to now + 365 days.
- Gates before every commit: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`.
- Git: each task on its own branch `feature/p2-m23-t{N}-{slug}` from `phase-2-lms-tracker`; commit there; never merge or push (the planner merges). Commit trailer: `Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>`.

## Spec amendments made by this plan

1. Progress stage `renewing` is dropped (renewal happens inside `ensure_session`); stages are `courses`, `items`, `saving`, `done`, `failed`, `login_required`.
2. A university whose sync starts with `ensure_session` returning `LoginRequired` ends as `login_required`; `Connection`/`Protocol` errors end as `failed`. An `Auth` error after a successful session check (during `list_courses`/`fetch_items`) sets `Expired` and ends as `login_required`.
3. Blackboard: attempts and user-grade endpoints that answer 404 are treated as "no attempts / no grade" (a column without a submission flow), not as a failed sync.
4. Duplicate `external_id`s inside one fetch (e.g. overlapping calendar chunks) are de-duplicated before diffing, keeping the first occurrence.
5. An item whose course changed on the platform (same `external_id`, different course) updates its `course_id`.
6. A due date appearing (None → date) or disappearing (date → None) counts as `due_changed`.

## Review Focus

1. **Assignment or quiz without a due date** (Moodle `duedate`/`timeclose` = 0, Blackboard no `grading.due`) → `due_at: None`, no panic, and None ↔ date transitions are flagged `due_changed`. Pinned in Task 4 and Task 7.
2. **One university fails mid-sync** (network drop during `fetch_items`) → nothing written for it, `last_sync_outcome = failed` with a message, the other university still syncs, `lms-sync:finished` fires exactly once. Pinned in Task 9.
3. **The platform returns the same item twice** (overlapping calendar chunks, duplicated rows) → one stored item, no UNIQUE violation. Pinned in Task 5 (chunk boundary) and Task 7 (dedupe).
4. **Grade text with HTML or comma decimals** (Moodle `gradefordisplay` like `<span>8,50</span> / 10,00`) → display text without tags, score parsed when possible, no failure when it cannot be parsed. Pinned in Task 4.
5. **A course is untracked between syncs** → its items are neither fetched nor marked removed, and its notes survive; they disappear from `get_tracker` only. Pinned in Task 7 and Task 8.

## Task order

`T1 → T2 (controller + user) → T3 → T4 → T5 → T6 → T7 → T8 → T9 → T10`. Strictly sequential (T4/T5 share `lms/mod.rs`; T6 touches every `LmsClient` implementation).

---

### Task 1: Fixture capture tool

**Files:**
- Create: `crates/infra/examples/lms_capture.rs`

**Interfaces:**
- Consumes: `academic_core::domain::{lms_credential_key, LmsPlatform}`, `academic_core::ports::SecretStore`, `academic_infra::secrets::KeyringSecretStore`, `academic_infra::lms::blackboard::decode_session`, `academic_infra::factory::http_client`.
- Produces: an example binary that writes redacted JSON to `crates/infra/tests/fixtures/lms/{moodle|blackboard}/<name>.json`, used by Tasks 4 and 5.

The tool calls the raw platform endpoints with the credential the app stored (read-only), redacts personal text and writes one file per endpoint. It never prints credentials or response bodies.

- [ ] **Step 1: Write the redaction function with tests first** (inside the example file, `#[cfg(test)] mod tests`, run with `cargo test -p academic-infra --example lms_capture`):

```rust
/// Keys whose string values are always replaced, even when they look like ids.
const PERSONAL_KEYS: [&str; 12] = [
    "fullname", "firstname", "lastname", "username", "userName", "email",
    "given", "family", "idnumber", "shortname", "displayname", "sitename",
];
/// Keys whose numeric values are personal results and get a fixed value.
const RESULT_KEY_FRAGMENTS: [&str; 3] = ["grade", "score", "mark"];
const REDACTED_TEXT: &str = "redacted";
const REDACTED_NUMBER: f64 = 7.5;

/// Replaces personal data while keeping the JSON shape, ids, enum-like values,
/// timestamps and URLs: any string containing whitespace or longer than 60
/// characters becomes "redacted"; strings under personal keys always do; numbers
/// under grade/score/mark keys become 7.5 (unless they are 0, -1 or ids named `id`/`*id`).
fn redact(value: &mut serde_json::Value, key: Option<&str>) { /* implement */ }

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn keeps_shape_ids_enums_and_timestamps() {
        let mut v = json!({
            "id": "_20_1", "status": "Completed", "due": "2026-10-15T21:59:00.000Z",
            "scoreProviderHandle": "resource/x-bb-assignment", "duedate": 1791000000,
            "url": "https://uah.blackboard.com/ultra/courses/_1_1/outline"
        });
        let before = v.clone();
        redact(&mut v, None);
        assert_eq!(v, before);
    }

    #[test]
    fn replaces_free_text_and_personal_keys() {
        let mut v = json!({
            "name": "Práctica 2 — Árboles AVL", "fullname": "Ana", "shortname": "CAL2",
            "intro": "x".repeat(80), "nested": [{ "title": "Examen parcial" }]
        });
        redact(&mut v, None);
        assert_eq!(v["name"], "redacted");
        assert_eq!(v["fullname"], "redacted");
        assert_eq!(v["shortname"], "redacted");
        assert_eq!(v["intro"], "redacted");
        assert_eq!(v["nested"][0]["title"], "redacted");
    }

    #[test]
    fn replaces_result_numbers_but_not_ids_or_sentinels() {
        let mut v = json!({ "grade": 8.25, "score": 10, "gradeid": 55, "possible": 10,
                            "bestgrade": 0, "rawgrade": -1, "displayGrade": { "score": 9 } });
        redact(&mut v, None);
        assert_eq!(v["grade"], 7.5);
        assert_eq!(v["score"], 7.5);
        assert_eq!(v["gradeid"], 55);
        assert_eq!(v["possible"], 10);
        assert_eq!(v["bestgrade"], 0);
        assert_eq!(v["rawgrade"], -1);
        assert_eq!(v["displayGrade"]["score"], 7.5);
    }
}
```

Implementation rules (exact): for objects, recurse with the child key; for arrays, recurse with the parent key; for strings: replace when `key` is in `PERSONAL_KEYS`, or the string contains whitespace, or its length > 60; for numbers: replace with `7.5` when `key` (lowercased) contains one of `RESULT_KEY_FRAGMENTS`, does not end with `id`, and the number is not `0` or `-1`; when the parent key is `displayGrade` (or `feedback`) apply the same number rule to `score`/`grade`. Booleans and nulls unchanged.

- [ ] **Step 2: Write the capture `main`.** Configuration through env vars (fail with a clear message if missing): `LMS_CAPTURE_UNIVERSITY_ID`, `LMS_CAPTURE_PLATFORM` (`moodle`|`blackboard`), `LMS_CAPTURE_BASE_URL`, optional `LMS_CAPTURE_MAX_COURSES` (default 3). Read the credential with `KeyringSecretStore.get(&lms_credential_key(platform, id))`. Use `http_client()` and `url::form_urlencoded::Serializer` for form bodies and query strings. Write each response, redacted and pretty-printed, to `crates/infra/tests/fixtures/lms/<platform>/<name>.json` (create the directory). Print only `wrote <path> (<n> bytes)` per file and a final count.

Moodle (POST `{base}/webservice/rest/server.php`, fields `wstoken`, `wsfunction`, `moodlewsrestformat=json`; arrays as `courseids[0]=…`):

| File | Call |
|---|---|
| `courses.json` | `core_enrol_get_users_courses(userid)` (userid from an in-memory `core_webservice_get_site_info` call that is NOT written) |
| `assignments.json` | `mod_assign_get_assignments(courseids = first MAX courses)` |
| `submission_status_{n}.json` | `mod_assign_get_submission_status(assignid)` for up to 3 assignments (prefer ones with `duedate > 0`) |
| `quizzes.json` | `mod_quiz_get_quizzes_by_courses(courseids)` |
| `quiz_attempts_{n}.json` / `quiz_best_grade_{n}.json` | `mod_quiz_get_user_attempts(quizid, status=finished)` / `mod_quiz_get_user_best_grade(quizid)` for up to 2 quizzes |
| `calendar_events.json` | `core_calendar_get_calendar_events(events[courseids][i], options[userevents]=0, options[siteevents]=0, options[timestart]=now−180d, options[timeend]=now+365d)` |

Blackboard (GET with `Cookie` header from `decode_session`; follow nothing, first page only):

| File | Path |
|---|---|
| `courses.json` | `/learn/api/public/v1/users/me/courses?expand=course&limit=100` |
| `columns_{n}.json` | `/learn/api/public/v2/courses/{courseId}/gradebook/columns` for the first MAX courses |
| `user_grades_{n}.json` | `/learn/api/public/v1/courses/{courseId}/gradebook/users/{userId}` (userId from an in-memory `/v1/users/me` call that is NOT written) |
| `attempts_{n}_{m}.json` | `/learn/api/public/v2/courses/{courseId}/gradebook/columns/{columnId}/attempts?userId={userId}` for up to 3 columns with a `contentId` per course |
| `calendar_items.json` | `/learn/api/public/v1/calendars/items?type=Course&since={now−56d}&until={now+56d}` |

A non-2xx response is written as `{"httpStatus": <code>}` instead of the body, so the tool never stops halfway.

- [ ] **Step 3: Gates and commit** (the example is compiled by `cargo clippy --all-targets`; do not run it — the controller runs it in Task 2):

```bash
git add crates/infra/examples/lms_capture.rs
git commit -m "test(infra): add LMS fixture capture tool with redaction"
```

---

### Task 2: Capture fixtures from the real accounts (controller + user)

Not a subagent task. The controller, with the user's prior consent:
1. Runs `cargo run -p academic-infra --example lms_capture` once per university (env vars as in Task 1; university ids from the app).
2. Checks the written files for leftover personal data (`grep` for whitespace-containing strings and known names), and lists the files and sizes to the user.
3. Commits the fixtures on branch `feature/p2-m23-t2-fixtures` (`test: add redacted LMS API fixtures`) and merges it.
4. Records in the ledger any surprising shape (e.g. different `scoreProviderHandle` strings, empty endpoints) and carries it into Tasks 4–5 dispatches.

If capture fails for a platform, Tasks 4/5 still proceed with synthetic tests only; the fixture smoke test for that platform is then skipped and the gap is reported to the user.

---

### Task 3: Item domain types and `effective_status`

**Files:**
- Modify: `crates/core/src/domain/lms.rs` (append types + tests)

**Interfaces:**
- Produces: `ItemKind`, `PlatformStatus`, `Grade`, `ItemChange`, `LmsItem`, `EffectiveStatus`, `effective_status`, `SyncWindow` (+ `SyncWindow::around`), `RemoteCourseRef`, `RemoteItem`, `TrackedItem`, `TrackerQuery`.

- [ ] **Step 1: Write failing tests** (in the existing `tests` module of `domain/lms.rs`):

```rust
fn item(kind: ItemKind, status: PlatformStatus, due: Option<DateTime<Utc>>) -> LmsItem {
    LmsItem {
        id: 1, university_id: 1, course_id: 1, external_id: "assign:1".into(), kind,
        title: "t".into(), due_at: due, url: None, platform_status: status, grade: None,
        change: None, removed_at: None, done: false, hidden: false, note: None,
        first_seen_at: Utc::now(), updated_at: Utc::now(),
    }
}

#[test]
fn effective_status_follows_the_spec_order() {
    let now = Utc::now();
    let past = Some(now - chrono::Duration::days(1));
    let future = Some(now + chrono::Duration::days(1));
    let mut graded = item(ItemKind::Assignment, PlatformStatus::Submitted, past);
    graded.grade = Some(Grade { display: "8 / 10".into(), score: Some(8.0), max: Some(10.0) });
    assert_eq!(effective_status(&graded, now), EffectiveStatus::Graded);
    assert_eq!(effective_status(&item(ItemKind::Quiz, PlatformStatus::Graded, None), now), EffectiveStatus::Graded);
    assert_eq!(effective_status(&item(ItemKind::Assignment, PlatformStatus::Submitted, past), now), EffectiveStatus::Submitted);
    let mut done = item(ItemKind::Assignment, PlatformStatus::NotSubmitted, past);
    done.done = true;
    assert_eq!(effective_status(&done, now), EffectiveStatus::Submitted);
    assert_eq!(effective_status(&item(ItemKind::Event, PlatformStatus::NotApplicable, past), now), EffectiveStatus::PastEvent);
    assert_eq!(effective_status(&item(ItemKind::Event, PlatformStatus::NotApplicable, future), now), EffectiveStatus::Pending);
    assert_eq!(effective_status(&item(ItemKind::Assignment, PlatformStatus::NotSubmitted, past), now), EffectiveStatus::Overdue);
    assert_eq!(effective_status(&item(ItemKind::Assignment, PlatformStatus::NotSubmitted, future), now), EffectiveStatus::Pending);
    assert_eq!(effective_status(&item(ItemKind::Assignment, PlatformStatus::NotSubmitted, None), now), EffectiveStatus::Pending);
}

#[test]
fn sync_window_spans_180_days_back_and_365_ahead() {
    let now = Utc::now();
    let w = SyncWindow::around(now);
    assert_eq!(w.from, now - chrono::Duration::days(180));
    assert_eq!(w.to, now + chrono::Duration::days(365));
}

#[test]
fn item_enums_round_trip() {
    for k in [ItemKind::Assignment, ItemKind::Quiz, ItemKind::Test, ItemKind::Event, ItemKind::Other] {
        assert_eq!(ItemKind::from_str(k.as_str()).unwrap(), k);
    }
    for s in [PlatformStatus::NotSubmitted, PlatformStatus::Submitted, PlatformStatus::Graded, PlatformStatus::NotApplicable] {
        assert_eq!(PlatformStatus::from_str(s.as_str()).unwrap(), s);
    }
    assert_eq!(PlatformStatus::NotApplicable.as_str(), "not_applicable");
    assert_eq!(serde_json::to_value(EffectiveStatus::PastEvent).unwrap(), "past_event");
}
```

- [ ] **Step 2: Implement** (append to `domain/lms.rs`):

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ItemKind { Assignment, Quiz, Test, Event, Other }
enum_str!(ItemKind { Assignment => "assignment", Quiz => "quiz", Test => "test", Event => "event", Other => "other" });

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlatformStatus { NotSubmitted, Submitted, Graded, NotApplicable }
enum_str!(PlatformStatus {
    NotSubmitted => "not_submitted", Submitted => "submitted",
    Graded => "graded", NotApplicable => "not_applicable",
});

/// A grade as the platform shows it, plus the numbers when available.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Grade { pub display: String, pub score: Option<f64>, pub max: Option<f64> }

/// Badge shown until the user dismisses it (spec §7).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ItemChange { New, DueChanged { previous_due_at: Option<DateTime<Utc>> } }

/// A stored deliverable, exam or course event.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LmsItem {
    pub id: i64,
    pub university_id: UniversityId,
    pub course_id: i64,
    pub external_id: String,
    pub kind: ItemKind,
    pub title: String,
    pub due_at: Option<DateTime<Utc>>,
    pub url: Option<String>,
    pub platform_status: PlatformStatus,
    pub grade: Option<Grade>,
    pub change: Option<ItemChange>,
    pub removed_at: Option<DateTime<Utc>>,
    pub done: bool,
    pub hidden: bool,
    pub note: Option<String>,
    pub first_seen_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EffectiveStatus { Pending, Overdue, Submitted, Graded, PastEvent }

/// Derived status (spec §4.1): graded › submitted/done › past event › overdue › pending.
pub fn effective_status(item: &LmsItem, now: DateTime<Utc>) -> EffectiveStatus {
    let past = item.due_at.is_some_and(|due| due < now);
    if item.grade.is_some() || item.platform_status == PlatformStatus::Graded {
        EffectiveStatus::Graded
    } else if item.platform_status == PlatformStatus::Submitted || item.done {
        EffectiveStatus::Submitted
    } else if item.kind == ItemKind::Event && past {
        EffectiveStatus::PastEvent
    } else if past {
        EffectiveStatus::Overdue
    } else {
        EffectiveStatus::Pending
    }
}

/// Date range used for course events.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SyncWindow { pub from: DateTime<Utc>, pub to: DateTime<Utc> }

impl SyncWindow {
    pub fn around(now: DateTime<Utc>) -> Self {
        Self { from: now - chrono::Duration::days(180), to: now + chrono::Duration::days(365) }
    }
}

/// A tracked course to fetch items for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteCourseRef { pub external_id: String }

/// An item as returned by the platform, before it is stored.
#[derive(Debug, Clone, PartialEq)]
pub struct RemoteItem {
    pub course_external_id: String,
    pub external_id: String,
    pub kind: ItemKind,
    pub title: String,
    pub due_at: Option<DateTime<Utc>>,
    pub url: Option<String>,
    pub platform_status: PlatformStatus,
    pub grade: Option<Grade>,
}

/// An item with the names the tracker shows next to it.
#[derive(Debug, Clone, PartialEq)]
pub struct TrackedItem {
    pub item: LmsItem,
    pub university_name: String,
    pub course_name: String,
    pub course_short_name: Option<String>,
}

/// Filter for the tracker list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TrackerQuery {
    pub university_id: Option<UniversityId>,
    pub include_hidden: bool,
    pub include_removed: bool,
}
```

(`rustfmt` will reflow the one-line enums; that is expected.)

- [ ] **Step 3: Gates and commit** — `git commit -m "feat(core): add LMS item domain and effective status"`.

---

### Task 4: Moodle `fetch_items` (inherent method)

**Files:**
- Modify: `crates/infra/src/lms/moodle.rs`
- Modify: `crates/infra/src/lms/mod.rs` (shared helpers below)
- Test: `crates/infra/tests/lms_moodle_items.rs`

**Interfaces:**
- Consumes (Task 3): `RemoteCourseRef`, `RemoteItem`, `SyncWindow`, `ItemKind`, `PlatformStatus`, `Grade`.
- Produces: `impl MoodleClient { pub async fn fetch_items(&self, courses: &[RemoteCourseRef], window: &SyncWindow) -> Result<Vec<RemoteItem>, LmsError> }` and, in `lms/mod.rs`, `pub(crate) fn strip_tags(&str) -> String`, `pub(crate) fn format_number(f64) -> String`, `pub(crate) fn unix_to_utc(i64) -> Option<DateTime<Utc>>`. Task 6 wires the method into the `LmsClient` trait.

- [ ] **Step 1: Shared helpers with unit tests** in `lms/mod.rs`:

```rust
/// Text of an HTML fragment: tags removed, `&nbsp;`/`&amp;` decoded, whitespace collapsed.
pub(crate) fn strip_tags(html: &str) -> String { /* implement */ }

/// `8.5` → "8.5", `10.0` → "10", `7.25` → "7.25" (at most 2 decimals, trailing zeros dropped).
pub(crate) fn format_number(value: f64) -> String { /* implement */ }

/// Moodle timestamps: 0 or negative means "no date".
pub(crate) fn unix_to_utc(seconds: i64) -> Option<DateTime<Utc>> {
    (seconds > 0).then(|| Utc.timestamp_opt(seconds, 0).single()).flatten()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_tags_keeps_text_only() {
        assert_eq!(strip_tags("<span class=\"x\">8,50</span> / 10,00"), "8,50 / 10,00");
        assert_eq!(strip_tags("A&nbsp;&amp; B\n  C"), "A & B C");
        assert_eq!(strip_tags("plain"), "plain");
    }

    #[test]
    fn format_number_drops_trailing_zeros() {
        assert_eq!(format_number(10.0), "10");
        assert_eq!(format_number(8.5), "8.5");
        assert_eq!(format_number(7.256), "7.26");
    }

    #[test]
    fn non_positive_timestamps_are_no_date() {
        assert_eq!(unix_to_utc(0), None);
        assert_eq!(unix_to_utc(-5), None);
        assert_eq!(unix_to_utc(1_791_000_000).unwrap().timestamp(), 1_791_000_000);
    }
}
```

- [ ] **Step 2: Write failing wiremock tests** — `crates/infra/tests/lms_moodle_items.rs`. Use synthetic JSON (exact shapes below), a mock per `wsfunction` (matched with `body_string_contains("wsfunction=…")`), and one fixture smoke test:

```rust
// Helpers: `mock_fn(server, function, body)` mounts POST /webservice/rest/server.php
// matching `wsfunction=<function>`; `client(server)` builds MoodleClient with token "tok".

#[tokio::test]
async fn maps_assignments_with_status_grade_and_missing_due_date() {
    // mod_assign_get_assignments → two assignments in course 12:
    //   {id: 5, cmid: 50, name: "P1", duedate: 1791000000, grade: 10, nosubmissions: 0}
    //   {id: 6, cmid: 60, name: "P2", duedate: 0,          grade: 10, nosubmissions: 0}
    // mod_assign_get_submission_status(assignid=5) →
    //   {"lastattempt": {"submission": {"status": "submitted"}},
    //    "feedback": {"gradefordisplay": "<span>8,50</span> / 10,00", "grade": {"grade": "8.50000"}}}
    // mod_assign_get_submission_status(assignid=6) → {"lastattempt": {"submission": {"status": "new"}}}
    // mod_quiz_get_quizzes_by_courses → {"quizzes": []}
    // core_calendar_get_calendar_events → {"events": []}
    let items = client(&server).fetch_items(&[course("12")], &window()).await.unwrap();
    let p1 = items.iter().find(|i| i.external_id == "assign:5").unwrap();
    assert_eq!(p1.course_external_id, "12");
    assert_eq!(p1.kind, ItemKind::Assignment);
    assert_eq!(p1.due_at.unwrap().timestamp(), 1_791_000_000);
    assert_eq!(p1.url.as_deref(), Some(format!("{}/mod/assign/view.php?id=50", server.uri()).as_str()));
    assert_eq!(p1.platform_status, PlatformStatus::Graded);
    let grade = p1.grade.as_ref().unwrap();
    assert_eq!(grade.display, "8,50 / 10,00");
    assert_eq!((grade.score, grade.max), (Some(8.5), Some(10.0)));
    let p2 = items.iter().find(|i| i.external_id == "assign:6").unwrap();
    assert_eq!((p2.due_at, p2.platform_status, p2.grade.clone()), (None, PlatformStatus::NotSubmitted, None));
}

#[tokio::test]
async fn team_submissions_and_offline_assignments() {
    // assignment 7: lastattempt.teamsubmission.status = "submitted" (submission absent) → Submitted
    // assignment 8: nosubmissions = 1, no feedback → NotApplicable
}

#[tokio::test]
async fn maps_quizzes_with_finished_attempts_and_best_grade() {
    // mod_quiz_get_quizzes_by_courses → {"quizzes": [
    //   {"id": 3, "course": 12, "coursemodule": 30, "name": "Q1", "timeclose": 1791000000, "grade": 10},
    //   {"id": 4, "course": 12, "coursemodule": 40, "name": "Q2", "timeclose": 0, "grade": 10}]}
    // attempts(quizid=3) → {"attempts": [{"id": 1, "state": "finished"}]}; best(3) → {"hasgrade": true, "grade": 7.25}
    // attempts(quizid=4) → {"attempts": []};                                   best(4) → {"hasgrade": false}
    // expect quiz:3 Graded "7.25 / 10" score 7.25 max 10, url /mod/quiz/view.php?id=30;
    //        quiz:4 NotSubmitted, due None, no grade
}

#[tokio::test]
async fn keeps_only_course_and_group_events_without_a_module() {
    // events: {id 90 eventtype "course" modulename null} kept; {id 91 eventtype "group" modulename ""} kept;
    //         {id 92 eventtype "due" modulename "assign"} dropped; {id 93 eventtype "user"} dropped
    // expect event:90 and event:91, kind Event, NotApplicable, due = timestart
    // also assert the request body contains "events%5Bcourseids%5D%5B0%5D=12" and "options%5Buserevents%5D=0"
}

#[tokio::test]
async fn any_failed_call_fails_the_whole_fetch() {
    // submission_status answers {"exception": "x", "errorcode": "invalidtoken"} → Err(LmsError::Auth)
}

#[tokio::test]
async fn non_numeric_course_ids_are_rejected() {
    // fetch_items(&[course("abc")]) → Err(LmsError::Protocol(_)) with no request made
}

#[tokio::test]
async fn captured_fixtures_parse() {
    // Skip (return early with eprintln!) when crates/infra/tests/fixtures/lms/moodle/assignments.json is absent.
    // Mount assignments.json, quizzes.json, calendar_events.json, every submission_status_*.json for any
    // assignid (fallback: first file), quiz_attempts_*/quiz_best_grade_* likewise.
    // Course ids: the ids present in assignments.json.
    // Assert: fetch_items succeeds and returns at least one item; every external_id is unique.
}
```

Write the comment-described tests out fully (mock bodies exactly as shown). Run → FAIL.

- [ ] **Step 3: Implement** in `moodle.rs`. Add the deserialization structs (all unknown fields ignored; `#[serde(default)]` on optional numbers/vecs):

```rust
#[derive(Deserialize)] struct AssignmentsResponse { #[serde(default)] courses: Vec<AssignCourse> }
#[derive(Deserialize)] struct AssignCourse { id: i64, #[serde(default)] assignments: Vec<Assignment> }
#[derive(Deserialize)] struct Assignment {
    id: i64, cmid: i64, name: String,
    #[serde(default)] duedate: i64, #[serde(default)] grade: f64, #[serde(default)] nosubmissions: i64,
}
#[derive(Deserialize)] struct SubmissionStatus { lastattempt: Option<LastAttempt>, feedback: Option<Feedback> }
#[derive(Deserialize)] struct LastAttempt { submission: Option<Submission>, teamsubmission: Option<Submission> }
#[derive(Deserialize)] struct Submission { status: String }
#[derive(Deserialize)] struct Feedback { gradefordisplay: Option<String>, grade: Option<FeedbackGrade> }
#[derive(Deserialize)] struct FeedbackGrade { grade: Option<String> }
#[derive(Deserialize)] struct QuizzesResponse { #[serde(default)] quizzes: Vec<Quiz> }
#[derive(Deserialize)] struct Quiz { id: i64, course: i64, coursemodule: i64, name: String,
    #[serde(default)] timeclose: i64, #[serde(default)] grade: f64 }
#[derive(Deserialize)] struct Attempts { #[serde(default)] attempts: Vec<serde_json::Value> }
#[derive(Deserialize)] struct BestGrade { #[serde(default)] hasgrade: bool, grade: Option<f64> }
#[derive(Deserialize)] struct EventsResponse { #[serde(default)] events: Vec<CalendarEvent> }
#[derive(Deserialize)] struct CalendarEvent { id: i64, name: String, courseid: i64,
    #[serde(default)] eventtype: String, modulename: Option<String>, #[serde(default)] timestart: i64 }
```

`call` currently takes `&[(&str, &str)]`; build owned `Vec<(String, String)>` for array params and pass `.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect::<Vec<_>>()`.

Mapping rules (exact):
- Course ids: parse every `external_id` as `i64`; any failure → `LmsError::Protocol("Invalid Moodle course id")` before any request. Empty `courses` → `Ok(vec![])` without requests.
- Assignments: `mod_assign_get_assignments` with `courseids[i]`. For each assignment, call `mod_assign_get_submission_status(assignid)`. Status: `submission` or else `teamsubmission` with `status == "submitted"` → `Submitted`; otherwise `NotSubmitted`; `nosubmissions == 1` → `NotApplicable` unless graded. Grade: present when `feedback.gradefordisplay` is non-empty after `strip_tags` → `platform_status = Graded`, `display` = stripped text, `score` = `feedback.grade.grade` parsed as `f64` (comma replaced by dot), `max` = `assignment.grade` when > 0. `due_at = unix_to_utc(duedate)`. `external_id = "assign:{id}"`, url `{base}/mod/assign/view.php?id={cmid}`, title = `name`.
- Quizzes: `mod_quiz_get_quizzes_by_courses(courseids)`; per quiz `mod_quiz_get_user_attempts(quizid, status=finished)` and `mod_quiz_get_user_best_grade(quizid)`. Any attempt → `Submitted`; `hasgrade && grade.is_some()` → `Graded`, `Grade { display: format!("{} / {}", format_number(score), format_number(max)), score, max }` (`max` = `quiz.grade` when > 0, display without " / max" otherwise). `external_id = "quiz:{id}"`, kind `Quiz`, url `{base}/mod/quiz/view.php?id={coursemodule}`, `due_at = unix_to_utc(timeclose)`.
- Events: `core_calendar_get_calendar_events` with `events[courseids][i]`, `options[userevents]=0`, `options[siteevents]=0`, `options[timestart]={window.from}`, `options[timeend]={window.to}` (unix seconds). Keep `eventtype` in {`course`, `group`} with `modulename` absent or empty. `external_id = "event:{id}"`, kind `Event`, `NotApplicable`, `due_at = unix_to_utc(timestart)`, url `{base}/calendar/view.php?view=day&time={timestart}`, `course_external_id = courseid`.
- Return order: assignments, quizzes, events. No logging of titles.

Run the test file → PASS; then gates.

- [ ] **Step 4: Commit** — `git commit -m "feat(infra): fetch Moodle assignments, quizzes and course events"`.

---

### Task 5: Blackboard `fetch_items` (inherent method)

**Files:**
- Modify: `crates/infra/src/lms/blackboard.rs`
- Test: `crates/infra/tests/lms_blackboard_items.rs`

**Interfaces:**
- Consumes: Task 3 types; Task 4 helpers `format_number` from `lms/mod.rs`.
- Produces: `impl BlackboardClient { pub async fn fetch_items(&self, courses: &[RemoteCourseRef], window: &SyncWindow) -> Result<Vec<RemoteItem>, LmsError> }`.

- [ ] **Step 1: Write failing wiremock tests** (synthetic shapes below; plus a fixture smoke test like Task 4's, reading `fixtures/lms/blackboard/columns_*.json`, `user_grades_*.json`, `attempts_*.json`, `calendar_items.json`, `courses.json`):

```rust
#[tokio::test]
async fn maps_columns_with_attempts_and_grades() {
    // GET /learn/api/public/v1/users/me → {"id": "_9_1", "userName": "u"}
    // GET /learn/api/public/v2/courses/_10_1/gradebook/columns → {"results": [
    //   {"id": "_1_1", "name": "Memoria", "contentId": "_100_1",
    //    "score": {"possible": 10}, "grading": {"type": "Attempts", "due": "2026-10-15T21:59:00.000Z"},
    //    "scoreProviderHandle": "resource/x-bb-assignment"},
    //   {"id": "_2_1", "name": "Test 1", "contentId": "_101_1", "score": {"possible": 20},
    //    "grading": {"type": "Attempts"}, "scoreProviderHandle": "resource/x-bb-asmt-test-link"},
    //   {"id": "_3_1", "name": "Total", "score": {"possible": 100}, "grading": {"type": "Calculated"}},
    //   {"id": "_4_1", "name": "Participación", "score": {"possible": 5}, "grading": {"type": "Manual"}}]}
    // GET /learn/api/public/v1/courses/_10_1/gradebook/users/_9_1 → {"results": [
    //   {"columnId": "_1_1", "score": 8.5, "status": "Graded"}]}
    // GET .../columns/_1_1/attempts?userId=_9_1 → {"results": [{"id": "a", "status": "Completed"}]}
    // GET .../columns/_2_1/attempts?userId=_9_1 → {"results": [{"id": "b", "status": "InProgress"}]}
    // GET /learn/api/public/v1/calendars/items?... → {"results": []}
    // expect: column:_1_1 Assignment, due 2026-10-15T21:59Z, Graded, grade "8.5 / 10" (8.5, 10);
    //         column:_2_1 Test, due None, NotSubmitted (only an in-progress attempt);
    //         column:_3_1 skipped (Calculated);
    //         column:_4_1 Other, NotSubmitted, no attempts request made (no contentId);
    //         url for all: {base}/ultra/courses/_10_1/outline
}

#[tokio::test]
async fn attempts_or_grades_answering_404_mean_none() {
    // user grades → 404, attempts for _1_1 → 404 → column:_1_1 NotSubmitted, no grade, fetch succeeds
}

#[tokio::test]
async fn other_http_errors_fail_the_fetch() {
    // columns → 500 → Err(LmsError::Protocol); columns → 401 → Err(LmsError::Auth)
}

#[tokio::test]
async fn course_events_are_fetched_in_chunks_and_deduplicated() {
    // window = 300 days → 3 chunks of ≤ 112 days (verify 3 calendar requests with increasing `since`)
    // every chunk returns {"results": [{"id": "_e1_1", "calendarId": "_10_1", "title": "Examen",
    //                                   "start": "2026-11-03T08:00:00.000Z", "type": "Course"},
    //                                  {"id": "_e2_1", "calendarId": "_other_1", "title": "x",
    //                                   "start": "2026-11-04T08:00:00.000Z", "type": "Course"}]}
    // expect exactly one calendar:_e1_1 (Event, NotApplicable, due = start); _e2_1 dropped (untracked calendar)
}
```

Write all four out fully. Run → FAIL.

- [ ] **Step 2: Implement** in `blackboard.rs`:
  - A `get_all_or_empty<T>(&self, path)` variant of `get_all` that maps an HTTP 404 on the first page to `Ok(vec![])` (refactor `get` so the 404 check is reusable; keep a single implementation of the request/response handling — no duplicated request code).
  - Query strings built with `url::form_urlencoded::Serializer` (`userId`, `type`, `since`, `until`).
  - User id: `self.verify().await?.user_id` once per call (or the `/users/me` request directly).
  - Per course (in the given order):
    1. Columns: `get_all("/learn/api/public/v2/courses/{id}/gradebook/columns")`. Skip `grading.type == "Calculated"`.
    2. Grades: `get_all_or_empty("/learn/api/public/v1/courses/{id}/gradebook/users/{userId}")` → map `columnId → score (f64)`; a grade counts when `score` is present.
    3. Attempts (only for columns with a `contentId`): `get_all_or_empty(".../columns/{col}/attempts?userId={userId}")`; submitted when any attempt status is not one of `InProgress`, `NotAttempted`, `Abandoned`, `Canceled`.
    4. Item: `external_id = "column:{col.id}"`, `course_external_id` = course id, title = `name`, `due_at` = `grading.due` parsed as RFC 3339 (invalid → `Protocol`), kind from `scoreProviderHandle`: `resource/x-bb-assignment` → `Assignment`; any handle containing `test` or `asmt` → `Test`; otherwise `Other`. Status: grade → `Graded` with `Grade { display: "{score} / {possible}" via format_number (or just the score when possible ≤ 0), score, max }`; else submitted → `Submitted`; else `NotSubmitted`. url `{base}/ultra/courses/{courseId}/outline`.
  - Events: split `[window.from, window.to)` into consecutive chunks of at most 112 days; for each chunk `get_all("/learn/api/public/v1/calendars/items?type=Course&since=…&until=…")` (RFC 3339 with `Z`); keep items whose `calendarId` is one of the requested course ids; `external_id = "calendar:{id}"`, kind `Event`, `NotApplicable`, `due_at = start`, url `{base}/ultra/calendar`. De-duplicate by `external_id` (first wins).
  - Return order: per course columns, then events.

Run → PASS; gates.

- [ ] **Step 3: Commit** — `git commit -m "feat(infra): fetch Blackboard gradebook items and course events"`.

---

### Task 6: `LmsClient::fetch_items` in the port

**Files:**
- Modify: `crates/core/src/ports/lms.rs` (trait method; update the "M2 adds" doc)
- Modify: `crates/infra/src/lms/moodle.rs`, `crates/infra/src/lms/blackboard.rs` (trait impl delegates to the inherent methods; then make the inherent methods private or remove the duplication — the trait method must contain the logic or a one-line delegation, not a copy)
- Modify: every other `LmsClient` implementation: `crates/core/src/lms/session.rs` tests (`FakeClient`), `src-tauri/src/test_support.rs` (`FakeLmsClient` → `Ok(vec![])`)

**Interfaces:**
- Produces: `async fn fetch_items(&self, courses: &[RemoteCourseRef], window: &SyncWindow) -> Result<Vec<RemoteItem>, LmsError>;` on `LmsClient` with doc "All or nothing: any failed request fails the whole call."

- [ ] **Step 1:** Add the method, move the inherent implementations into the trait impls (or delegate), update fakes. Run `cargo test --workspace` → PASS. Gates.
- [ ] **Step 1b: Live check.** Extend `crates/infra/tests/lms_live.rs` with a second `#[ignore]` test `live_fetch_items` (same env vars): `list_courses`, take the first 3 courses, `fetch_items` with `SyncWindow::around(Utc::now())`, and print only counts per `kind` and per `platform_status` plus how many have a due date and a grade. Never print titles, ids or URLs.
- [ ] **Step 2: Commit** — `git commit -m "feat(core): add fetch_items to the LMS client port"`.

---

### Task 7: Pure diff and the item repository port

**Files:**
- Create: `crates/core/src/lms/diff.rs` (+ `pub use diff::*;` in `lms/mod.rs`)
- Modify: `crates/core/src/ports/repositories.rs` (`LmsItemRepository`)

**Interfaces:**
- Consumes: Task 3 types, `LmsCourse`.
- Produces:

```rust
/// A new item to insert.
#[derive(Debug, Clone, PartialEq)]
pub struct NewItem { pub course_id: i64, pub remote: RemoteItem, pub change: Option<ItemChange> }

/// Platform fields to overwrite on an existing item, plus the resulting badge.
/// Always clears `removed_at` (the item was returned again).
#[derive(Debug, Clone, PartialEq)]
pub struct ItemUpdate { pub id: i64, pub course_id: i64, pub remote: RemoteItem, pub change: Option<ItemChange> }

#[derive(Debug, Clone, PartialEq, Default)]
pub struct SyncChanges {
    pub inserts: Vec<NewItem>,
    pub updates: Vec<ItemUpdate>,
    /// Ids of items no longer returned by the platform (only those not already removed).
    pub removals: Vec<i64>,
    /// Courses whose items were fetched in this sync (their `items_synced_at` is set).
    pub synced_course_ids: Vec<i64>,
    pub new_items: u32,
    pub due_changed: u32,
    pub removed: u32,
}

/// Compares what the platform returned for `fetched_courses` with what is stored.
pub fn diff_items(fetched_courses: &[LmsCourse], stored: &[LmsItem], remote: &[RemoteItem]) -> SyncChanges;
```

```rust
/// Persistence for LMS items (spec §7, §8).
pub trait LmsItemRepository: Send + Sync {
    fn list_items_for_university(&self, university_id: UniversityId) -> RepoResult<Vec<LmsItem>>;
    /// One transaction: inserts (first_seen_at = updated_at = now), updates (platform fields,
    /// course, change; removed_at cleared; updated_at = now), removals (removed_at = now),
    /// and `items_synced_at = now` for `synced_course_ids`. Personal fields never change.
    fn apply_sync(&self, university_id: UniversityId, changes: &SyncChanges, now: DateTime<Utc>) -> RepoResult<()>;
    /// Items of tracked, available courses, ordered by due date (undated last), then title.
    fn query_items(&self, query: &TrackerQuery) -> RepoResult<Vec<TrackedItem>>;
    fn get_item(&self, id: i64) -> RepoResult<Option<TrackedItem>>;
    fn set_item_done(&self, id: i64, done: bool) -> RepoResult<TrackedItem>;
    fn set_item_hidden(&self, id: i64, hidden: bool) -> RepoResult<TrackedItem>;
    fn set_item_note(&self, id: i64, note: Option<&str>) -> RepoResult<TrackedItem>;
    fn dismiss_item_change(&self, id: i64) -> RepoResult<TrackedItem>;
    /// Clears every badge (of one university, or all). Returns rows changed.
    fn dismiss_all_changes(&self, university_id: Option<UniversityId>) -> RepoResult<u64>;
}
```

- [ ] **Step 1: Write failing diff tests** in `diff.rs` (`#[cfg(test)]`), with helpers `course(id, external, synced: bool)`, `stored(id, course_id, ext, due, change, removed)`, `remote(course_ext, ext, due)`:
  - `first_sync_of_a_course_inserts_without_badge` — course `items_synced_at: None` → insert with `change: None`, `new_items == 0`, `synced_course_ids == [course]`.
  - `later_sync_marks_new_items` — course synced before → `change: Some(New)`, `new_items == 1`.
  - `due_date_change_records_the_original_previous_date` — stored due D1, remote D2 → update `DueChanged { previous: D1 }`, `due_changed == 1`; then stored already `DueChanged{previous: D1}` with due D2, remote D3 → previous stays D1, counted again (`due_changed == 1`).
  - `returning_to_the_original_date_clears_the_badge` — stored `DueChanged{previous: D1}`, due D2, remote D1 → update with `change: None`, `due_changed == 0`.
  - `due_date_appearing_or_disappearing_is_a_change` — None→D1 and D1→None both produce `DueChanged`.
  - `new_badge_survives_a_due_change` — stored `change: New`, due changes → stays `New`, `due_changed == 0`.
  - `unchanged_items_produce_no_update` — identical remote → no update, no counts.
  - `other_field_changes_update_silently` — title/status/grade/url/kind change → update with the stored `change` kept, counts 0.
  - `course_change_updates_the_course` — same external id under another fetched course → update with new `course_id`.
  - `missing_items_are_removed_once` — stored not returned (course fetched) → `removals == [id]`, `removed == 1`; already removed → not listed again.
  - `reappearing_items_are_restored` — stored with `removed_at` returned → update (clears removal), counts 0.
  - `items_of_unfetched_courses_are_untouched` — stored item whose course is not in `fetched_courses` → no removal, no update.
  - `remote_duplicates_and_unknown_courses_are_ignored` — two remotes with the same external id → one insert (first wins); remote whose `course_external_id` is not a fetched course → ignored.

- [ ] **Step 2: Implement `diff_items`** following the spec §7 table and the amendments at the top of this plan (match on `(external_id)` within the university; build a `HashMap<&str, &LmsItem>` of stored items; course lookup by external id among `fetched_courses`; due comparison on `Option<DateTime<Utc>>`). Add `LmsItemRepository` to `repositories.rs`. Run tests → PASS; gates.
- [ ] **Step 3: Commit** — `git commit -m "feat(core): add LMS item diff and item repository port"`.

---

### Task 8: Migration 0003 and `LmsItemRepository` implementation

**Files:**
- Create: `crates/infra/migrations/0003_lms_items.sql`
- Modify: `crates/infra/src/db/mod.rs` (add `(3, include_str!("../../migrations/0003_lms_items.sql"))`, `mod items;`)
- Create: `crates/infra/src/db/items.rs`
- Test: `crates/infra/tests/db_items.rs`

**Interfaces:**
- Consumes: Task 7 (`LmsItemRepository`, `SyncChanges`, `NewItem`, `ItemUpdate`), Task 3 types.
- Produces: `impl LmsItemRepository for Database`.

- [ ] **Step 1: Migration** (exact):

```sql
CREATE TABLE lms_items (
  id INTEGER PRIMARY KEY,
  university_id INTEGER NOT NULL REFERENCES universities(id) ON DELETE CASCADE,
  course_id INTEGER NOT NULL REFERENCES courses(id) ON DELETE CASCADE,
  external_id TEXT NOT NULL,
  kind TEXT NOT NULL CHECK (kind IN ('assignment','quiz','test','event','other')),
  title TEXT NOT NULL,
  due_at TEXT,
  url TEXT,
  platform_status TEXT NOT NULL
    CHECK (platform_status IN ('not_submitted','submitted','graded','not_applicable')),
  grade_display TEXT, grade_score REAL, grade_max REAL,
  change_kind TEXT CHECK (change_kind IN ('new','due_changed')),
  previous_due_at TEXT,
  removed_at TEXT,
  done INTEGER NOT NULL DEFAULT 0,
  hidden INTEGER NOT NULL DEFAULT 0,
  note TEXT,
  first_seen_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  UNIQUE (university_id, external_id));
CREATE INDEX idx_lms_items_due ON lms_items (university_id, due_at);
```

Mapping: `grade` ↔ (`grade_display`, `grade_score`, `grade_max`), present when `grade_display` is not NULL; `change` ↔ (`change_kind`, `previous_due_at`) — `New` = ('new', NULL), `DueChanged { previous_due_at }` = ('due_changed', previous or NULL).

- [ ] **Step 2: Write failing tests** `db_items.rs` (helpers create a university and two courses via the existing repositories, then `set_tracked_courses` for one of them):
  - `apply_sync_inserts_updates_removes_and_marks_courses_synced` — insert two items, then a second `apply_sync` with one update (title + `DueChanged`), one removal; assert rows, `removed_at`, `updated_at` moved, `first_seen_at` unchanged, and `list_courses` shows `items_synced_at` set for the synced course only.
  - `personal_fields_survive_sync_updates` — set done/hidden/note, then an update → personal fields unchanged; update clears `removed_at`.
  - `query_items_filters_and_orders` — items in a tracked and an untracked course, one hidden, one removed, one undated: default query returns only tracked, visible, not removed, ordered by due (undated last); `include_hidden`/`include_removed` widen it; `university_id` filters; each `TrackedItem` carries university name, course name and short name.
  - `unavailable_courses_are_excluded` — after `upsert_remote_courses` drops a course (available = 0) its items are not returned.
  - `personal_setters_and_dismiss` — `set_item_done/hidden/note` return the updated item; note `None` clears; `dismiss_item_change` clears one; `dismiss_all_changes(Some(u))` returns the count and only touches that university; missing id → `RepoError::NotFound`.
  - `grade_and_change_round_trip` — `Grade { display, score: Some, max: None }` and `DueChanged { previous_due_at: None }` survive a write/read.
  - `deleting_a_university_cascades_to_items`.
  - `apply_sync_is_atomic` — a `SyncChanges` whose second insert violates UNIQUE (same external id twice) → error and nothing from that call is stored.

- [ ] **Step 3: Implement `db/items.rs`** following `db/universities.rs` / `db/courses.rs` conventions (raw row struct + conversion with `parse_enum`/`parse_datetime(_opt)`, `map_err`, `NotFound` on 0 changed rows, `conn.transaction()` for `apply_sync`, helper functions taking `&Connection` so setters can return the re-read `TrackedItem` without re-locking). `query_items` SQL joins `courses` (`tracked = 1 AND available = 1`) and `universities`, applies the filters, `ORDER BY i.due_at IS NULL, i.due_at, i.title, i.id`. Run tests → PASS; gates.
- [ ] **Step 4: Commit** — `git commit -m "feat(db): add LMS items table and repository"`.

---

### Task 9: `LmsSyncRunner`

**Files:**
- Modify: `crates/core/src/pipeline/mod.rs` (`mod lock;` → `pub(crate) mod lock;`) and `pipeline/lock.rs` (`RunLock`/`RunLockGuard` → `pub(crate)`)
- Modify: `crates/core/src/ports/lms.rs` (observer port)
- Create: `crates/core/src/lms/sync.rs` (+ `pub use sync::*;`)
- Test: `crates/core/tests/lms_sync.rs` (fakes in a new `crates/core/tests/support/lms.rs`, declared from `support/mod.rs`; mark it `#[allow(dead_code)]` if other test binaries that include `support` do not use it)

**Interfaces:**
- Consumes: `ensure_session`, `diff_items`, all LMS ports and repositories, `RunLock`, `Clock`.
- Produces:

```rust
// ports/lms.rs
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LmsSyncStage { Courses, Items, Saving, Done, Failed, LoginRequired }

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LmsSyncProgress { pub university_id: UniversityId, pub university_name: String, pub stage: LmsSyncStage }

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LmsSyncResult {
    pub university_id: UniversityId, pub university_name: String, pub outcome: SyncOutcome,
    pub new_items: u32, pub due_changed: u32, pub removed: u32, pub error: Option<String>,
}

pub trait LmsSyncObserver: Send + Sync {
    fn progress(&self, progress: &LmsSyncProgress);
    /// Called exactly once per executed sync.
    fn finished(&self, results: &[LmsSyncResult]);
}

// lms/sync.rs
#[derive(Debug, thiserror::Error)]
pub enum LmsSyncError {
    #[error("A sync is already running")] AlreadyRunning,
    #[error("Sign in to a university first")] NothingToSync,
    #[error("Storage error: {0}")] Storage(String),
}

pub struct LmsRepositories {
    pub universities: Arc<dyn UniversityRepository>,
    pub courses: Arc<dyn CourseRepository>,
    pub items: Arc<dyn LmsItemRepository>,
}

pub struct LmsSyncRunner { /* repos, factory, authenticator, observer, clock, lock: RunLock */ }

pub struct PreparedSync { universities: Vec<LmsUniversity>, _lock: RunLockGuard }

impl LmsSyncRunner {
    pub fn new(repos: LmsRepositories, factory: Arc<dyn LmsClientFactory>,
               authenticator: Arc<dyn LmsAuthenticator>, observer: Arc<dyn LmsSyncObserver>,
               clock: Arc<dyn Clock>) -> Self;
    /// Targets: the given university, or every university; universities with
    /// `login_state == Never` are excluded. Empty target set → `NothingToSync`;
    /// missing id → `NothingToSync`. Takes the lock (→ `AlreadyRunning`).
    pub fn prepare(&self, university_id: Option<UniversityId>) -> Result<PreparedSync, LmsSyncError>;
    pub async fn execute(&self, prepared: PreparedSync) -> Vec<LmsSyncResult>;
    pub fn is_running(&self) -> bool;
}
```

`RunLock` gains `pub(crate) fn is_busy(&self) -> bool` (with a unit test) for `is_running`.

- [ ] **Step 1: Write failing tests** `crates/core/tests/lms_sync.rs` with in-memory fakes (repositories backed by `Mutex<Vec<…>>`, a scripted client whose `list_courses`/`fetch_items` results are set per university, an authenticator returning a scripted result, a recording observer, a fixed clock):
  - `syncs_courses_and_items_and_records_success` — one connected university with one tracked course: `list_courses` → upsert called; `fetch_items` called with exactly the tracked+available course refs and `SyncWindow::around(now)`; `apply_sync` receives the diff; `record_sync(Success, None)`; result counts match; stages `Courses → Items → Saving → Done`; `finished` once.
  - `no_tracked_courses_skips_fetch_but_succeeds` — `fetch_items` not called, outcome `Success`, zero counts.
  - `login_required_before_fetch` — session check yields `LoginRequired` → outcome `LoginRequired`, `record_sync(LoginRequired, Some(msg))`, stage `LoginRequired`, no `apply_sync`.
  - `auth_error_during_fetch_marks_expired` — `fetch_items` → `Auth` → `set_login(Expired, label kept)`, outcome `LoginRequired`.
  - `connection_error_fails_only_that_university` — two universities, the first `fetch_items` → `Connection`: first `Failed` with the error message and no `apply_sync`; second `Success`; `finished` once with two results in order.
  - `never_signed_in_universities_are_skipped` and `prepare_errors` — all `Never` → `NothingToSync`; unknown id → `NothingToSync`; second `prepare` while the first `PreparedSync` is alive → `AlreadyRunning`; after `execute` returns, `prepare` works again and `is_running()` is false.
  - `storage_failure_is_a_failed_outcome` — `apply_sync` returns `Storage` → `Failed`.

- [ ] **Step 2: Implement `execute`** per university, sequentially:
  1. `progress(Courses)`; `ensure_session(&u, factory, authenticator, universities)`: `Ok(updated)` → continue with `updated`; `Err(LoginRequired)` → outcome `LoginRequired`; `Err(Lms(e))` → `Failed(e)`; `Err(Storage|Secret)` → `Failed`.
  2. `factory.create(&u)?.list_courses()` → `courses.upsert_remote_courses(u.id, &remote, now)`.
  3. `tracked = courses.list_courses(u.id)` filtered `tracked && available`. Empty → skip to 5 with empty changes.
  4. `progress(Items)`; `fetch_items(&refs, &SyncWindow::around(now))`.
  5. `progress(Saving)`; `stored = items.list_items_for_university(u.id)`; `changes = diff_items(&tracked, &stored, &remote)`; `items.apply_sync(u.id, &changes, now)`.
  6. `record_sync(u.id, now, Success, None)`; `progress(Done)`.
  - Any `LmsError::Auth` in steps 2–4 → `set_login(Expired, u.account_label)` and outcome `LoginRequired`. Any other error → `Failed` with `error.to_string()` (repository errors → `"Something went wrong while saving data"`). For non-success outcomes call `record_sync(u.id, now, outcome, Some(&message))` and emit `progress(Failed)` / `progress(LoginRequired)`. If `record_sync` itself fails, the result's outcome is `Failed` with error "Something went wrong while saving data" (`core` has no logging; never panic).
  - After all universities: drop the lock guard, then `observer.finished(&results)` exactly once.
- [ ] **Step 3:** Run tests → PASS; gates. **Commit** — `git commit -m "feat(core): add LMS sync runner"`.

---

### Task 10: Course, sync and tracker commands

**Files:**
- Create: `src-tauri/src/commands/tracker.rs` (sync, tracker, item commands), `src-tauri/src/lms_observer.rs` (`TauriLmsSyncObserver`)
- Modify: `src-tauri/src/commands/universities.rs` (course commands + sign-in race fix), `src-tauri/src/commands/mod.rs`, `src-tauri/src/dto/lms.rs` (new DTOs), `src-tauri/src/state.rs` (`items`, `lms_sync: Arc<LmsSyncRunner>`), `src-tauri/src/setup.rs`, `src-tauri/src/lib.rs`, `src-tauri/src/error.rs` (`From<LmsSyncError>`), `src-tauri/src/validation.rs` (note), `src-tauri/src/test_support.rs` (sync observer fake; harness builds the runner)

**Interfaces:**
- Consumes: Tasks 3–9.
- Produces — commands and DTOs used by M4 (exact JSON):

```ts
interface LmsCourseDto { id: number; universityId: number; name: string; shortName: string | null;
  url: string | null; tracked: boolean; available: boolean; isNew: boolean /* !reviewed */ }
interface GradeDto { display: string; score: number | null; max: number | null }
type ItemChangeDto = { kind: 'new' } | { kind: 'due_changed'; previousDueAt: string | null };
interface LmsItemDto { id: number; universityId: number; universityName: string; courseId: number;
  courseName: string; courseShortName: string | null; externalId: string;
  kind: 'assignment' | 'quiz' | 'test' | 'event' | 'other'; title: string; dueAt: string | null;
  url: string | null; status: 'pending' | 'overdue' | 'submitted' | 'graded' | 'past_event';
  grade: GradeDto | null; change: ItemChangeDto | null; removedAt: string | null;
  done: boolean; hidden: boolean; note: string | null }
interface TrackerQuery { universityId: number | null; includeHidden: boolean; includeRemoved: boolean }
interface TrackerDto { items: LmsItemDto[]; universities: LmsUniversityDto[]; syncRunning: boolean }
interface LmsSyncResultDto { universityId: number; universityName: string;
  outcome: 'success' | 'failed' | 'login_required'; newItems: number; dueChanged: number;
  removed: number; error: string | null }
// events
// "lms-sync:progress" { universityId, universityName, stage: 'courses'|'items'|'saving'|'done'|'failed'|'login_required' }
// "lms-sync:finished" { results: LmsSyncResultDto[] }
```

| Command | Args (JS keys) | Returns | Notes |
|---|---|---|---|
| `lms_fetch_courses` | `{ id }` | `LmsCourseDto[]` | requires `loginState == ok` (else `auth` "Sign in to {name} first"); `factory.create` + `list_courses` + `upsert_remote_courses`; `Auth` from the platform → `set_login(Expired)` and `auth` error |
| `get_courses` | `{ universityId }` | `LmsCourseDto[]` | missing university → `not_found` |
| `set_tracked_courses` | `{ universityId, courseIds }` | `LmsCourseDto[]` | |
| `lms_sync` | `{ universityId: number \| null }` | `null` | `prepare` then spawn `execute` with a supervisor (if the task panics, emit `lms-sync:finished` with `[]`) — same pattern as `commands/runs.rs::start_check` |
| `get_tracker` | `{ query: TrackerQuery }` | `TrackerDto` | `status` = `effective_status(item, clock.now())` |
| `set_item_done` / `set_item_hidden` | `{ id, value }` | `LmsItemDto` | |
| `set_item_note` | `{ id, note }` | `LmsItemDto` | trimmed; empty → null; > 2000 chars → `validation` "Notes can have at most 2000 characters" |
| `dismiss_item_change` | `{ id }` | `LmsItemDto` | |
| `dismiss_all_changes` | `{ universityId: number \| null }` | `number` | |

`LmsSyncError` → `CommandError`: `AlreadyRunning` → `conflict`, `NothingToSync` → `validation`, `Storage` → internal storage message.

- [ ] **Step 1: Sign-in race fix (carried over from M1 final review).** In `commands/universities.rs::complete_sign_in`, after `adopt_credential` succeeds, re-check that the university still exists with the same `platform` and `base_url`; if not, delete the just-stored secret (`secrets.delete(&lms_credential_key(captured.platform, captured.id))`) and return `not_found("University not found")`. Test: a fake factory whose `verify` deletes the university from the repository mid-call (or a test-only hook) — if that is impractical, test the helper `still_matches(state, captured) -> Result<bool, CommandError>` directly and document it in the report.
- [ ] **Step 2: Write failing command tests** in `commands/tracker.rs` and `commands/universities.rs` using the harness (fake LMS client scripted to return courses and items; extend `FakeLmsBehavior` with `courses: fn() -> Result<Vec<RemoteCourse>, LmsError>` and `items: fn() -> Result<Vec<RemoteItem>, LmsError>`, defaults `Ok(vec![])`):
  - `fetch_courses_requires_a_connected_university` / `fetch_courses_upserts_and_returns_dtos` / `fetch_courses_auth_error_marks_expired`.
  - `set_tracked_courses_returns_reviewed_courses`.
  - `sync_rejects_when_nothing_to_sync` (all `Never`) and `sync_twice_conflicts` (call the runner's `prepare` to hold the lock in the test).
  - `tracker_returns_items_with_status_and_names` — seed items through `apply_sync`, assert DTO JSON keys (`dueAt`, `status`, `courseShortName`, `change.kind`, `change.previousDueAt`) and `status` values.
  - `note_validation_and_personal_setters` — 2001-char note → `validation`; `"  "` → null; done/hidden round trip; dismiss single and all.
  - `lms_sync_error_mapping` in `error.rs`.
  - DTO JSON shape tests in `dto/lms.rs` for `LmsItemDto` (both change variants and `null`), `LmsCourseDto`, `LmsSyncResultDto`.
- [ ] **Step 3: Implement**: DTOs (`From`/`new` builders from domain types; timestamps as `Option<DateTime<Utc>>` like `LmsUniversityDto`), `TauriLmsSyncObserver` (events `lms-sync:progress` / `lms-sync:finished`, payload structs camelCase, logs `university_id` + stage only, never titles), state wiring (`AppState.items`, `AppState.lms_sync`; `LmsSyncRunner::new` in `AppState::new` from the same `db` and collaborators plus a new `lms_observer: Arc<dyn LmsSyncObserver>` collaborator), command registration in `lib.rs`. `get_tracker`'s `universities` reuses the existing `universities(state)` helper.
- [ ] **Step 4:** Run all gates. **Commit** — `git commit -m "feat(app): add course, sync and tracker commands"`.

---

## After M2+M3

The planner merges every task branch, runs the gates, and (with the user's consent) runs `live_fetch_items` for both universities to confirm real data maps without errors; only counts are printed. M4 (dashboard, wizard course step, Universities page completion) and M5 (email integration, docs) are planned next, informed by the real items the sync stores.
