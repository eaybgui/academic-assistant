# Phase 2 · M1 — LMS sign-in gate: Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let the user add a Moodle or Blackboard university, sign in through the university's SSO in an embedded window, store a validated scoped credential, check and silently renew the session — and validate all of it on the user's desktop before any other Phase 2 work starts.

**Architecture:** New LMS domain types, ports and the `ensure_session`/`adopt_credential` logic live in `crates/core` (pure). `crates/infra` gets migration `0002`, the university/course repositories, the public platform probe and the Moodle/Blackboard clients (`verify`, `list_courses`). `src-tauri` gets the login-window capture (`lms_auth`), the `TauriLmsAuthenticator` and the university commands. The React app replaces the manual catalog page with a Universities page.

**Tech Stack:** Rust (Tauri 2.12, rusqlite, reqwest 0.13 without the `form` feature, wiremock), React 19 + TypeScript + React Query + Vitest.

**Spec:** `docs/superpowers/specs/2026-10-04-phase-2-lms-tracker-design.md` (read §1–§6, §8, §12, §14, §16 and **§18 amendments**, which override earlier sections).

## Global Constraints

- Follow `AGENTS.md`: implement only your task, no unrelated refactors, no new dependencies beyond those named in the task, report deviations instead of improvising.
- Dependency rule `src-tauri → infra → core`. `core` stays free of I/O (no `reqwest`, `rusqlite`, `url`, Tauri).
- Secrets (Moodle token, Blackboard cookies) only in `SecretStore` and in memory. Never in SQLite, logs, DTOs, events, error messages or test output. Never log URLs seen in the login window.
- Secret keys: `academic-assistant/university/{id}/moodle-token`, `academic-assistant/university/{id}/blackboard-session`.
- Credential Manager limit: a stored secret is at most 2560 bytes (UTF-8, stored with `set_secret`).
- Serialized enums are snake_case; DTO fields camelCase; timestamps RFC 3339 UTC.
- UI copy in English, sentence case. All frontend command calls go through `src/lib/api/`.
- Gates before every commit: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`; for frontend tasks also `npm run typecheck`, `npm run lint`, `npm test`.
- Git: each task works on its own branch `feature/p2-m1-t{N}-{slug}` created from `phase-2-lms-tracker`, commits there, and does not merge (the planner merges). Commit messages end with `Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>`.

## Review Focus

1. **The user closes the sign-in window or abandons it** → the command returns "Sign-in cancelled" (or "Sign-in timed out" after 10 minutes), the window is gone, `login_state` is unchanged and a retry works. Pinned in Task 6 (`complete_sign_in` with a cancelled capture) and Task 5 (window lifecycle checklist).
2. **The platform URL is pasted with a path, query, uppercase, spaces or `http://`** → it is normalized to the `https` origin or rejected with a clear message. Pinned in Task 1 (`normalize_base_url` tests).
3. **The same platform is added twice under a different name** → `conflict` "This platform is already added", no second row. Pinned in Task 6.
4. **The stored credential is missing or unreadable** (deleted from Credential Manager, corrupted JSON) → treated as an expired session (`Auth`), so silent renewal runs instead of an internal error. Pinned in Task 3 (factory tests) and Task 1 (`ensure_session` renewal tests).
5. **The captured Blackboard cookie set exceeds 2560 bytes** → a clear `auth` error naming the problem (cookie names logged, never values), instead of a Credential Manager failure. Pinned in Task 5 (`encode_session_cookies` size test).

## Task order and parallelism

`T1 → (T2 ∥ T3) → T4 → T5 → T7`, and `T6` (frontend) can run any time after T1 because it only depends on the DTO contract in Task 5's Interfaces block. T2 and T3 touch disjoint files.

---

### Task 1: Core LMS domain, ports and session logic

**Files:**
- Create: `crates/core/src/domain/lms.rs`
- Modify: `crates/core/src/domain/mod.rs` (add `mod lms; pub use lms::*;`)
- Modify: `crates/core/src/domain/secrets.rs` (add `lms_credential_key` + tests)
- Create: `crates/core/src/ports/lms.rs`
- Modify: `crates/core/src/ports/mod.rs` (add `mod lms; pub use lms::*;`)
- Modify: `crates/core/src/ports/repositories.rs` (add `UniversityRepository`, `CourseRepository`)
- Create: `crates/core/src/lms/mod.rs`, `crates/core/src/lms/url.rs`, `crates/core/src/lms/session.rs`
- Modify: `crates/core/src/lib.rs` (add `pub mod lms;`, update the crate doc to mention LMS tracking)

**Interfaces:**
- Consumes: `enum_str!` macro, `ErrorKind`, `RepoError`/`RepoResult`, `SecretStore`/`SecretError` (existing).
- Produces (exact names used by later tasks):
  - `domain::{UniversityId, LmsPlatform, LoginState, SyncOutcome, LmsUniversity, LmsCourse, LmsIdentity, RemoteCourse, lms_credential_key}`
  - `ports::{LmsError, LmsClient, LmsClientFactory, LmsAuthenticator, UniversityRepository, CourseRepository}`
  - `lms::{normalize_base_url, BaseUrlError, ensure_session, adopt_credential, SessionError}`

- [ ] **Step 1: Write the domain types**

`crates/core/src/domain/lms.rs`:

```rust
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::enum_str;

pub type UniversityId = i64;

/// Learning platform a university runs. Stored as `universities.platform`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LmsPlatform {
    Moodle,
    Blackboard,
}

enum_str!(LmsPlatform {
    Moodle => "moodle",
    Blackboard => "blackboard",
});

/// Whether the app holds a working credential for a university.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LoginState {
    Never,
    Ok,
    Expired,
    Error,
}

enum_str!(LoginState {
    Never => "never",
    Ok => "ok",
    Expired => "expired",
    Error => "error",
});

/// Outcome of the last sync of a university (written from M3 on).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SyncOutcome {
    Success,
    Failed,
    LoginRequired,
}

enum_str!(SyncOutcome {
    Success => "success",
    Failed => "failed",
    LoginRequired => "login_required",
});

/// A university the user follows through its learning platform.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LmsUniversity {
    pub id: UniversityId,
    pub name: String,
    pub platform: LmsPlatform,
    /// Normalized `https` origin without trailing slash, e.g. `https://aulavirtual.uv.es`.
    pub base_url: String,
    pub login_state: LoginState,
    /// Name the platform reports for the signed-in user ("Connected as …").
    pub account_label: Option<String>,
    pub last_sync_at: Option<DateTime<Utc>>,
    pub last_sync_outcome: Option<SyncOutcome>,
    pub last_sync_error: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// A course discovered on a university's platform.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LmsCourse {
    pub id: i64,
    pub university_id: UniversityId,
    /// Platform course id: Moodle `"1234"`, Blackboard `"_10_1"`.
    pub external_id: String,
    pub name: String,
    pub short_name: Option<String>,
    pub url: Option<String>,
    pub tracked: bool,
    /// False when the platform stopped returning the course.
    pub available: bool,
    /// False until the user saves a course selection that includes it.
    pub reviewed: bool,
    /// None until the course's items are synced for the first time (M3).
    pub items_synced_at: Option<DateTime<Utc>>,
}

/// The signed-in user as reported by the platform.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LmsIdentity {
    pub user_id: String,
    pub display_name: String,
}

/// A course as returned by the platform, before it is stored.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteCourse {
    pub external_id: String,
    pub name: String,
    pub short_name: Option<String>,
    pub url: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn enums_round_trip_through_their_strings() {
        for p in [LmsPlatform::Moodle, LmsPlatform::Blackboard] {
            assert_eq!(LmsPlatform::from_str(p.as_str()).unwrap(), p);
        }
        for s in [LoginState::Never, LoginState::Ok, LoginState::Expired, LoginState::Error] {
            assert_eq!(LoginState::from_str(s.as_str()).unwrap(), s);
        }
        for o in [SyncOutcome::Success, SyncOutcome::Failed, SyncOutcome::LoginRequired] {
            assert_eq!(SyncOutcome::from_str(o.as_str()).unwrap(), o);
        }
        assert_eq!(SyncOutcome::LoginRequired.as_str(), "login_required");
    }

    #[test]
    fn platform_serializes_snake_case() {
        assert_eq!(serde_json::to_value(LmsPlatform::Blackboard).unwrap(), "blackboard");
    }
}
```

- [ ] **Step 2: Add the secret key helper** in `crates/core/src/domain/secrets.rs`:

```rust
use super::lms::{LmsPlatform, UniversityId};

/// Secret store key for a university's LMS credential (Moodle web-service token or
/// Blackboard session cookies as JSON).
pub fn lms_credential_key(platform: LmsPlatform, id: UniversityId) -> String {
    let suffix = match platform {
        LmsPlatform::Moodle => "moodle-token",
        LmsPlatform::Blackboard => "blackboard-session",
    };
    format!("academic-assistant/university/{id}/{suffix}")
}
```

Test (inside the existing `tests` module):

```rust
#[test]
fn lms_credential_key_depends_on_platform_and_id() {
    assert_eq!(
        lms_credential_key(LmsPlatform::Moodle, 3),
        "academic-assistant/university/3/moodle-token"
    );
    assert_eq!(
        lms_credential_key(LmsPlatform::Blackboard, 4),
        "academic-assistant/university/4/blackboard-session"
    );
}
```

- [ ] **Step 3: Write the ports** — `crates/core/src/ports/lms.rs`:

```rust
use async_trait::async_trait;

use crate::domain::{ErrorKind, LmsIdentity, LmsUniversity, RemoteCourse};

/// Error returned by an LMS client, factory or authenticator. Every message is
/// user-facing and never contains a token, cookie or URL from the login window.
#[derive(Debug, thiserror::Error)]
pub enum LmsError {
    /// Missing, unreadable, expired or rejected credential: sign-in is needed.
    #[error("{0}")]
    Auth(String),
    #[error("{0}")]
    Connection(String),
    /// Unexpected response or a platform function that is not available.
    #[error("{0}")]
    Protocol(String),
}

impl LmsError {
    pub fn kind(&self) -> ErrorKind {
        match self {
            LmsError::Auth(_) => ErrorKind::Auth,
            LmsError::Connection(_) => ErrorKind::Connection,
            LmsError::Protocol(_) => ErrorKind::Protocol,
        }
    }
}

/// Read-only access to one university's platform as the signed-in user.
/// M2 adds `fetch_items`.
#[async_trait]
pub trait LmsClient: Send + Sync {
    /// Who the credential belongs to; also checks the platform exposes what we need.
    async fn verify(&self) -> Result<LmsIdentity, LmsError>;
    async fn list_courses(&self) -> Result<Vec<RemoteCourse>, LmsError>;
}

pub trait LmsClientFactory: Send + Sync {
    /// Builds a client with the credential stored in `SecretStore`. A missing or
    /// unreadable credential is `LmsError::Auth`.
    fn create(&self, university: &LmsUniversity) -> Result<Box<dyn LmsClient>, LmsError>;
    /// Builds a client with a credential that is not stored yet (used to verify a
    /// freshly captured credential before storing it).
    fn create_with_credential(
        &self,
        university: &LmsUniversity,
        credential: &str,
    ) -> Result<Box<dyn LmsClient>, LmsError>;
}

/// Renews a university's credential without user interaction (hidden login window).
#[async_trait]
pub trait LmsAuthenticator: Send + Sync {
    /// On success the new credential is already verified and stored.
    /// `Err(LmsError::Auth)` when the user has to sign in interactively.
    async fn renew_silently(&self, university: &LmsUniversity) -> Result<(), LmsError>;
}
```

- [ ] **Step 4: Add the repository traits** to `crates/core/src/ports/repositories.rs` (import the new domain types):

```rust
/// Persistence for LMS universities.
pub trait UniversityRepository: Send + Sync {
    fn list_universities(&self) -> RepoResult<Vec<LmsUniversity>>;
    fn get_university(&self, id: UniversityId) -> RepoResult<Option<LmsUniversity>>;
    /// Duplicate name or base URL → `RepoError::Conflict`.
    fn insert_university(
        &self,
        name: &str,
        platform: LmsPlatform,
        base_url: &str,
        now: DateTime<Utc>,
    ) -> RepoResult<LmsUniversity>;
    fn rename_university(&self, id: UniversityId, name: &str) -> RepoResult<LmsUniversity>;
    /// Cascades to its courses (and, from M3, items). Missing → `NotFound`.
    fn delete_university(&self, id: UniversityId) -> RepoResult<()>;
    fn set_login(
        &self,
        id: UniversityId,
        state: LoginState,
        account_label: Option<&str>,
    ) -> RepoResult<LmsUniversity>;
    fn record_sync(
        &self,
        id: UniversityId,
        at: DateTime<Utc>,
        outcome: SyncOutcome,
        error: Option<&str>,
    ) -> RepoResult<()>;
}

/// Persistence for courses discovered on LMS platforms.
pub trait CourseRepository: Send + Sync {
    /// Ordered by name.
    fn list_courses(&self, university_id: UniversityId) -> RepoResult<Vec<LmsCourse>>;
    /// In one transaction: inserts unknown courses (untracked, unreviewed, available),
    /// updates name/short name/url of known ones, marks every returned course
    /// available and every other course of the university unavailable.
    /// Missing university → `NotFound`.
    fn upsert_remote_courses(
        &self,
        university_id: UniversityId,
        courses: &[RemoteCourse],
        now: DateTime<Utc>,
    ) -> RepoResult<()>;
    /// Exactly `course_ids` become tracked; every course of the university becomes
    /// reviewed. An id that is not a course of this university → `NotFound` and no
    /// change. Returns the university's courses.
    fn set_tracked_courses(
        &self,
        university_id: UniversityId,
        course_ids: &[i64],
    ) -> RepoResult<Vec<LmsCourse>>;
}
```

- [ ] **Step 5: Write failing tests for URL normalization** — `crates/core/src/lms/url.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_only_the_https_origin() {
        assert_eq!(normalize_base_url("aulavirtual.uv.es").unwrap(), "https://aulavirtual.uv.es");
        assert_eq!(
            normalize_base_url(" https://aulavirtual.uv.es/my/courses.php?x=1#top ").unwrap(),
            "https://aulavirtual.uv.es"
        );
        assert_eq!(
            normalize_base_url("HTTPS://UAH.Blackboard.com/ultra/stream").unwrap(),
            "https://uah.blackboard.com"
        );
        assert_eq!(normalize_base_url("https://host.example:8443/x").unwrap(), "https://host.example:8443");
    }

    #[test]
    fn rejects_empty_non_https_and_malformed_urls() {
        assert_eq!(normalize_base_url("   "), Err(BaseUrlError::Empty));
        assert_eq!(normalize_base_url("http://aulavirtual.uv.es"), Err(BaseUrlError::NotHttps));
        assert_eq!(normalize_base_url("ftp://x.es"), Err(BaseUrlError::NotHttps));
        assert_eq!(normalize_base_url("https://"), Err(BaseUrlError::Invalid));
        assert_eq!(normalize_base_url("https://user@host.es"), Err(BaseUrlError::Invalid));
        assert_eq!(normalize_base_url("https://ex ample.es"), Err(BaseUrlError::Invalid));
        assert_eq!(normalize_base_url("https://nodot"), Err(BaseUrlError::Invalid));
    }

    #[test]
    fn error_messages_are_user_facing() {
        assert_eq!(BaseUrlError::Empty.to_string(), "Enter the platform URL");
        assert_eq!(BaseUrlError::NotHttps.to_string(), "The platform URL must start with https://");
        assert_eq!(BaseUrlError::Invalid.to_string(), "Enter a valid platform URL");
    }
}
```

Run: `cargo test -p academic-core lms::url` → Expected: compile error (function missing).

- [ ] **Step 6: Implement** (top of the same file):

```rust
//! Platform URL normalization (spec §5.3).

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BaseUrlError {
    #[error("Enter the platform URL")]
    Empty,
    #[error("The platform URL must start with https://")]
    NotHttps,
    #[error("Enter a valid platform URL")]
    Invalid,
}

/// Reduces user input to an `https` origin (`https://host[:port]`, lowercase host,
/// no path, query, fragment or trailing slash). Input without a scheme gets `https://`.
pub fn normalize_base_url(input: &str) -> Result<String, BaseUrlError> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(BaseUrlError::Empty);
    }
    let rest = match trimmed.split_once("://") {
        Some((scheme, rest)) if scheme.eq_ignore_ascii_case("https") => rest,
        Some(_) => return Err(BaseUrlError::NotHttps),
        None => trimmed,
    };
    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
    let host = authority.split(':').next().unwrap_or_default();
    let valid = !authority.is_empty()
        && !authority.contains('@')
        && !authority.contains(char::is_whitespace)
        && host.contains('.')
        && !host.starts_with('.')
        && !host.ends_with('.');
    if !valid {
        return Err(BaseUrlError::Invalid);
    }
    Ok(format!("https://{}", authority.to_ascii_lowercase()))
}
```

Run: `cargo test -p academic-core lms::url` → Expected: PASS.

- [ ] **Step 7: Write failing tests for the session logic** — `crates/core/src/lms/session.rs` test module. The fakes live in the test module:

```rust
#[cfg(test)]
mod tests {
    use std::collections::VecDeque;
    use std::sync::Mutex;

    use async_trait::async_trait;
    use chrono::{DateTime, Utc};

    use super::*;
    use crate::domain::{LmsPlatform, RemoteCourse, SyncOutcome};
    use crate::ports::{LmsClient, RepoError, RepoResult, SecretError};

    fn university(state: LoginState) -> LmsUniversity {
        LmsUniversity {
            id: 7,
            name: "UAH".into(),
            platform: LmsPlatform::Blackboard,
            base_url: "https://uah.blackboard.com".into(),
            login_state: state,
            account_label: Some("Old Name".into()),
            last_sync_at: None,
            last_sync_outcome: None,
            last_sync_error: None,
            created_at: Utc::now(),
        }
    }

    fn identity() -> LmsIdentity {
        LmsIdentity { user_id: "_1_1".into(), display_name: "Ana Pérez".into() }
    }

    /// Each `verify()` pops the next scripted result (shared by every client built).
    #[derive(Default)]
    struct FakeFactory {
        verify_results: Mutex<VecDeque<Result<LmsIdentity, LmsError>>>,
        credentials_seen: Mutex<Vec<String>>,
    }
    struct FakeClient(Result<LmsIdentity, LmsError>);

    #[async_trait]
    impl LmsClient for FakeClient {
        async fn verify(&self) -> Result<LmsIdentity, LmsError> {
            match &self.0 {
                Ok(id) => Ok(id.clone()),
                Err(LmsError::Auth(m)) => Err(LmsError::Auth(m.clone())),
                Err(LmsError::Connection(m)) => Err(LmsError::Connection(m.clone())),
                Err(LmsError::Protocol(m)) => Err(LmsError::Protocol(m.clone())),
            }
        }
        async fn list_courses(&self) -> Result<Vec<RemoteCourse>, LmsError> {
            Ok(vec![])
        }
    }
    impl FakeFactory {
        fn scripted(results: Vec<Result<LmsIdentity, LmsError>>) -> Self {
            Self { verify_results: Mutex::new(results.into()), ..Default::default() }
        }
        fn next(&self) -> Box<dyn LmsClient> {
            let result = self.verify_results.lock().unwrap().pop_front().expect("scripted result");
            Box::new(FakeClient(result))
        }
    }
    impl LmsClientFactory for FakeFactory {
        fn create(&self, _u: &LmsUniversity) -> Result<Box<dyn LmsClient>, LmsError> {
            Ok(self.next())
        }
        fn create_with_credential(&self, _u: &LmsUniversity, c: &str) -> Result<Box<dyn LmsClient>, LmsError> {
            self.credentials_seen.lock().unwrap().push(c.to_string());
            Ok(self.next())
        }
    }

    struct FakeAuthenticator {
        result: Mutex<Option<Result<(), LmsError>>>,
        calls: Mutex<u32>,
    }
    impl FakeAuthenticator {
        fn returning(result: Result<(), LmsError>) -> Self {
            Self { result: Mutex::new(Some(result)), calls: Mutex::new(0) }
        }
    }
    #[async_trait]
    impl LmsAuthenticator for FakeAuthenticator {
        async fn renew_silently(&self, _u: &LmsUniversity) -> Result<(), LmsError> {
            *self.calls.lock().unwrap() += 1;
            self.result.lock().unwrap().take().expect("renewal scripted once")
        }
    }

    /// Records `set_login` calls; returns the university with the new state.
    struct FakeUniversities {
        base: LmsUniversity,
        logins: Mutex<Vec<(LoginState, Option<String>)>>,
    }
    impl FakeUniversities {
        fn new(base: LmsUniversity) -> Self {
            Self { base, logins: Mutex::new(vec![]) }
        }
    }
    impl UniversityRepository for FakeUniversities {
        fn list_universities(&self) -> RepoResult<Vec<LmsUniversity>> { unreachable!() }
        fn get_university(&self, _id: i64) -> RepoResult<Option<LmsUniversity>> { unreachable!() }
        fn insert_university(&self, _n: &str, _p: LmsPlatform, _b: &str, _t: DateTime<Utc>) -> RepoResult<LmsUniversity> { unreachable!() }
        fn rename_university(&self, _id: i64, _n: &str) -> RepoResult<LmsUniversity> { unreachable!() }
        fn delete_university(&self, _id: i64) -> RepoResult<()> { unreachable!() }
        fn set_login(&self, _id: i64, state: LoginState, label: Option<&str>) -> RepoResult<LmsUniversity> {
            self.logins.lock().unwrap().push((state, label.map(str::to_string)));
            Ok(LmsUniversity { login_state: state, account_label: label.map(str::to_string), ..self.base.clone() })
        }
        fn record_sync(&self, _id: i64, _a: DateTime<Utc>, _o: SyncOutcome, _e: Option<&str>) -> RepoResult<()> { unreachable!() }
    }

    #[derive(Default)]
    struct FakeSecrets {
        stored: Mutex<Vec<(String, String)>>,
        fail: bool,
    }
    impl SecretStore for FakeSecrets {
        fn get(&self, _k: &str) -> Result<Option<String>, SecretError> { Ok(None) }
        fn set(&self, k: &str, v: &str) -> Result<(), SecretError> {
            if self.fail { return Err(SecretError("locked".into())); }
            self.stored.lock().unwrap().push((k.into(), v.into()));
            Ok(())
        }
        fn delete(&self, _k: &str) -> Result<(), SecretError> { Ok(()) }
    }

    #[tokio::test]
    async fn valid_session_marks_ok_with_the_platform_name() {
        let u = university(LoginState::Ok);
        let factory = FakeFactory::scripted(vec![Ok(identity())]);
        let auth = FakeAuthenticator::returning(Ok(()));
        let repo = FakeUniversities::new(u.clone());
        let updated = ensure_session(&u, &factory, &auth, &repo).await.unwrap();
        assert_eq!(updated.login_state, LoginState::Ok);
        assert_eq!(updated.account_label.as_deref(), Some("Ana Pérez"));
        assert_eq!(*auth.calls.lock().unwrap(), 0);
    }

    #[tokio::test]
    async fn expired_session_is_renewed_silently_then_verified_again() {
        let u = university(LoginState::Ok);
        let factory = FakeFactory::scripted(vec![Err(LmsError::Auth("expired".into())), Ok(identity())]);
        let auth = FakeAuthenticator::returning(Ok(()));
        let repo = FakeUniversities::new(u.clone());
        let updated = ensure_session(&u, &factory, &auth, &repo).await.unwrap();
        assert_eq!(updated.login_state, LoginState::Ok);
        assert_eq!(*auth.calls.lock().unwrap(), 1);
    }

    #[tokio::test]
    async fn failed_silent_renewal_marks_expired_and_keeps_the_label() {
        let u = university(LoginState::Ok);
        let factory = FakeFactory::scripted(vec![Err(LmsError::Auth("expired".into()))]);
        let auth = FakeAuthenticator::returning(Err(LmsError::Auth("Sign-in required".into())));
        let repo = FakeUniversities::new(u.clone());
        let error = ensure_session(&u, &factory, &auth, &repo).await.unwrap_err();
        assert!(matches!(error, SessionError::LoginRequired(ref name) if name == "UAH"));
        assert_eq!(
            repo.logins.lock().unwrap().as_slice(),
            &[(LoginState::Expired, Some("Old Name".to_string()))]
        );
    }

    #[tokio::test]
    async fn auth_failure_after_renewal_marks_expired() {
        let u = university(LoginState::Ok);
        let factory = FakeFactory::scripted(vec![
            Err(LmsError::Auth("expired".into())),
            Err(LmsError::Auth("still rejected".into())),
        ]);
        let auth = FakeAuthenticator::returning(Ok(()));
        let repo = FakeUniversities::new(u.clone());
        assert!(matches!(
            ensure_session(&u, &factory, &auth, &repo).await,
            Err(SessionError::LoginRequired(_))
        ));
        assert_eq!(repo.logins.lock().unwrap()[0].0, LoginState::Expired);
    }

    #[tokio::test]
    async fn connection_errors_do_not_change_the_login_state() {
        let u = university(LoginState::Ok);
        let factory = FakeFactory::scripted(vec![Err(LmsError::Connection("offline".into()))]);
        let auth = FakeAuthenticator::returning(Ok(()));
        let repo = FakeUniversities::new(u.clone());
        assert!(matches!(
            ensure_session(&u, &factory, &auth, &repo).await,
            Err(SessionError::Lms(LmsError::Connection(_)))
        ));
        assert!(repo.logins.lock().unwrap().is_empty());
        assert_eq!(*auth.calls.lock().unwrap(), 0);
    }

    #[tokio::test]
    async fn never_signed_in_requires_login_without_any_request() {
        let u = university(LoginState::Never);
        let factory = FakeFactory::scripted(vec![]);
        let auth = FakeAuthenticator::returning(Ok(()));
        let repo = FakeUniversities::new(u.clone());
        assert!(matches!(
            ensure_session(&u, &factory, &auth, &repo).await,
            Err(SessionError::LoginRequired(_))
        ));
        assert_eq!(*auth.calls.lock().unwrap(), 0);
    }

    #[tokio::test]
    async fn adopt_credential_verifies_before_storing() {
        let u = university(LoginState::Never);
        let factory = FakeFactory::scripted(vec![Ok(identity())]);
        let secrets = FakeSecrets::default();
        let id = adopt_credential(&u, "[{\"name\":\"a\",\"value\":\"b\"}]", &factory, &secrets).await.unwrap();
        assert_eq!(id.display_name, "Ana Pérez");
        assert_eq!(
            secrets.stored.lock().unwrap()[0].0,
            "academic-assistant/university/7/blackboard-session"
        );
    }

    #[tokio::test]
    async fn rejected_credential_is_never_stored() {
        let u = university(LoginState::Never);
        let factory = FakeFactory::scripted(vec![Err(LmsError::Auth("rejected".into()))]);
        let secrets = FakeSecrets::default();
        assert!(adopt_credential(&u, "x", &factory, &secrets).await.is_err());
        assert!(secrets.stored.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn secret_store_failure_is_reported() {
        let u = university(LoginState::Never);
        let factory = FakeFactory::scripted(vec![Ok(identity())]);
        let secrets = FakeSecrets { fail: true, ..Default::default() };
        assert!(matches!(
            adopt_credential(&u, "x", &factory, &secrets).await,
            Err(SessionError::Secret(_))
        ));
    }

    #[test]
    fn repo_errors_become_storage_errors() {
        assert!(matches!(SessionError::from(RepoError::NotFound), SessionError::Storage(_)));
    }
}
```

Run: `cargo test -p academic-core lms::session` → Expected: compile error.

- [ ] **Step 8: Implement the session logic** (top of `session.rs`):

```rust
//! Session check and credential adoption shared by sign-in, "Check connection" and
//! (from M3) sync (spec §6.4, §6.5, §18.4).

use crate::domain::{lms_credential_key, LmsIdentity, LmsUniversity, LoginState};
use crate::ports::{
    LmsAuthenticator, LmsClientFactory, LmsError, RepoError, SecretError, SecretStore,
    UniversityRepository,
};

#[derive(Debug, thiserror::Error)]
pub enum SessionError {
    /// The user has to sign in interactively. Holds the university name.
    #[error("{0} needs you to sign in again")]
    LoginRequired(String),
    #[error(transparent)]
    Lms(#[from] LmsError),
    #[error("Storage error: {0}")]
    Storage(String),
    #[error("Secure storage error: {0}")]
    Secret(String),
}

impl From<RepoError> for SessionError {
    fn from(error: RepoError) -> Self {
        SessionError::Storage(error.to_string())
    }
}

impl From<SecretError> for SessionError {
    fn from(error: SecretError) -> Self {
        SessionError::Secret(error.0)
    }
}

/// Verifies the stored credential. On `Auth`, tries one silent renewal and verifies
/// again. Updates `login_state` (`Ok` with the platform's display name, or `Expired`
/// keeping the previous label). Connection/protocol errors leave the state unchanged.
pub async fn ensure_session(
    university: &LmsUniversity,
    factory: &dyn LmsClientFactory,
    authenticator: &dyn LmsAuthenticator,
    universities: &dyn UniversityRepository,
) -> Result<LmsUniversity, SessionError> {
    if university.login_state == LoginState::Never {
        return Err(SessionError::LoginRequired(university.name.clone()));
    }
    match verify(university, factory).await {
        Ok(identity) => return mark_ok(universities, university, &identity),
        Err(LmsError::Auth(_)) => {}
        Err(other) => return Err(other.into()),
    }
    match authenticator.renew_silently(university).await {
        Ok(()) => {}
        Err(LmsError::Auth(_)) => return mark_expired(universities, university),
        Err(other) => return Err(other.into()),
    }
    match verify(university, factory).await {
        Ok(identity) => mark_ok(universities, university, &identity),
        Err(LmsError::Auth(_)) => mark_expired(universities, university),
        Err(other) => Err(other.into()),
    }
}

/// Verifies a freshly captured credential and, only if the platform accepts it,
/// stores it. Does not change `login_state` (the caller decides).
pub async fn adopt_credential(
    university: &LmsUniversity,
    credential: &str,
    factory: &dyn LmsClientFactory,
    secrets: &dyn SecretStore,
) -> Result<LmsIdentity, SessionError> {
    let identity = factory
        .create_with_credential(university, credential)?
        .verify()
        .await?;
    secrets.set(&lms_credential_key(university.platform, university.id), credential)?;
    Ok(identity)
}

async fn verify(
    university: &LmsUniversity,
    factory: &dyn LmsClientFactory,
) -> Result<LmsIdentity, LmsError> {
    factory.create(university)?.verify().await
}

fn mark_ok(
    universities: &dyn UniversityRepository,
    university: &LmsUniversity,
    identity: &LmsIdentity,
) -> Result<LmsUniversity, SessionError> {
    Ok(universities.set_login(university.id, LoginState::Ok, Some(&identity.display_name))?)
}

fn mark_expired(
    universities: &dyn UniversityRepository,
    university: &LmsUniversity,
) -> Result<LmsUniversity, SessionError> {
    universities.set_login(
        university.id,
        LoginState::Expired,
        university.account_label.as_deref(),
    )?;
    Err(SessionError::LoginRequired(university.name.clone()))
}
```

`crates/core/src/lms/mod.rs`:

```rust
//! LMS tracking logic (Phase 2): platform URL normalization and session handling.

mod session;
mod url;

pub use session::*;
pub use url::*;
```

Run: `cargo test -p academic-core` → Expected: all PASS (existing tests included).

- [ ] **Step 9: Gates and commit**

Run the Rust gates from Global Constraints. Then:

```bash
git add crates/core
git commit -m "feat(core): add LMS domain, ports and session logic"
```

---

### Task 2: Migration 0002, university/course repositories and catalog retirement

**Files:**
- Create: `crates/infra/migrations/0002_lms.sql`
- Modify: `crates/infra/src/db/mod.rs` (migration list; register new modules)
- Create: `crates/infra/src/db/universities.rs`, `crates/infra/src/db/courses.rs`
- Modify: `crates/infra/src/db/catalog.rs` (only `catalog()`, reading the new schema)
- Modify: `crates/core/src/ports/repositories.rs` (`CatalogRepository` keeps only `catalog()`)
- Modify: `crates/core/tests/support/store.rs` (drop the CRUD impls and their bookkeeping fields)
- Modify: `src-tauri/src/commands/catalog.rs` (keep only `get_catalog`; delete CRUD commands, helpers and their tests), `src-tauri/src/lib.rs` (unregister them)
- Delete: `crates/infra/tests/db_catalog.rs`
- Create: `crates/infra/tests/db_lms.rs`
- Modify: `crates/infra/tests/db_results.rs` (the test at ~line 413 that used `add_university`/`add_course`)
- Modify: `crates/infra/tests/db_migrations.rs` (Phase 1 → 0002 upgrade test)

**Interfaces:**
- Consumes (Task 1): `LmsUniversity`, `LmsCourse`, `LmsPlatform`, `LoginState`, `SyncOutcome`, `RemoteCourse`, `UniversityRepository`, `CourseRepository`.
- Produces: `impl UniversityRepository for Database`, `impl CourseRepository for Database`; `CatalogRepository::catalog()` returns all universities (id, name) and the **tracked and available** courses with `aliases = [short_name]` when the short name is non-empty and differs from the name.

- [ ] **Step 1: Write the migration** `crates/infra/migrations/0002_lms.sql`:

```sql
-- Phase 2: universities become LMS connections; courses are discovered from the
-- platform. The manual catalog is discarded: deleting the rows lets the existing
-- foreign keys null out ai_results.university_id/course_id and
-- accounts.default_university_id before the tables are recreated.
DELETE FROM courses;
DELETE FROM universities;
DROP TABLE courses;
DROP TABLE universities;

CREATE TABLE universities (
  id INTEGER PRIMARY KEY,
  name TEXT NOT NULL UNIQUE,
  platform TEXT NOT NULL CHECK (platform IN ('moodle','blackboard')),
  base_url TEXT NOT NULL UNIQUE,
  login_state TEXT NOT NULL DEFAULT 'never'
    CHECK (login_state IN ('never','ok','expired','error')),
  account_label TEXT,
  last_sync_at TEXT,
  last_sync_outcome TEXT CHECK (last_sync_outcome IN ('success','failed','login_required')),
  last_sync_error TEXT,
  created_at TEXT NOT NULL);

CREATE TABLE courses (
  id INTEGER PRIMARY KEY,
  university_id INTEGER NOT NULL REFERENCES universities(id) ON DELETE CASCADE,
  external_id TEXT NOT NULL,
  name TEXT NOT NULL,
  short_name TEXT,
  url TEXT,
  tracked INTEGER NOT NULL DEFAULT 0,
  available INTEGER NOT NULL DEFAULT 1,
  reviewed INTEGER NOT NULL DEFAULT 0,
  items_synced_at TEXT,
  created_at TEXT NOT NULL,
  UNIQUE (university_id, external_id));
```

- [ ] **Step 2: Write the failing upgrade test** in `crates/infra/tests/db_migrations.rs` (replace the existing `open_in_memory_starts_with_an_empty_catalog` import if needed; keep the idempotency test):

```rust
use academic_core::ports::{AccountRepository, CatalogRepository, ResultRepository, UniversityRepository};
use rusqlite::Connection;

const MIGRATION_0001: &str = include_str!("../migrations/0001_init.sql");

/// A database as Phase 1 left it: schema version 1, a manual university and course,
/// an account defaulting to that university and a result linked to both.
fn phase_one_database(path: &std::path::Path) {
    let conn = Connection::open(path).unwrap();
    conn.execute_batch(MIGRATION_0001).unwrap();
    conn.execute_batch(
        "CREATE TABLE schema_migrations (version INTEGER PRIMARY KEY, applied_at TEXT NOT NULL);
         INSERT INTO schema_migrations VALUES (1, '2026-09-28T00:00:00Z');
         INSERT INTO universities (id, name, created_at) VALUES (1, 'UV', '2026-09-28T00:00:00Z');
         INSERT INTO courses (id, university_id, name, aliases, created_at)
           VALUES (1, 1, 'Algebra', '[]', '2026-09-28T00:00:00Z');
         INSERT INTO accounts (id, name, email_address, provider_type, config, folders,
           default_university_id, initial_sync_days, enabled, created_at, updated_at)
           VALUES (1, 'UV mail', 'me@uv.es', 'imap',
             '{\"host\":\"imap.uv.es\",\"port\":993,\"username\":\"me\"}', '[\"INBOX\"]',
             1, 14, 1, '2026-09-28T00:00:00Z', '2026-09-28T00:00:00Z');
         INSERT INTO runs (id, trigger, started_at, status) VALUES (1, 'manual', '2026-09-28T00:00:00Z', 'success');
         INSERT INTO processed_messages (id, account_id, folder, provider_message_id, content_hash,
           received_at, outcome, run_id, processed_at)
           VALUES (1, 1, 'INBOX', '42', 'h', '2026-09-28T00:00:00Z', 'analyzed', 1, '2026-09-28T00:00:00Z');
         INSERT INTO ai_results (id, account_id, processed_message_id, run_id, detected_at, received_at,
           subject, sender_email, university_id, course_id, category, priority, action_required,
           possible_change, model_provider, model_name)
           VALUES (1, 1, 1, 1, '2026-09-28T00:00:00Z', '2026-09-28T00:00:00Z', 'Exam', 'p@uv.es',
             1, 1, 'exam', 'high', 1, 0, 'ollama', 'qwen3:8b');",
    )
    .unwrap();
}

#[test]
fn upgrading_a_phase_one_database_discards_the_manual_catalog_but_keeps_mail_data() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("app.db");
    phase_one_database(&path);

    let db = academic_infra::db::Database::open(&path).unwrap();

    assert!(db.list_universities().unwrap().is_empty());
    assert!(db.catalog().unwrap().courses.is_empty());
    let account = AccountRepository::get(&db, 1).unwrap().unwrap();
    assert_eq!(account.default_university_id, None);
    let result = ResultRepository::get(&db, 1).unwrap().unwrap();
    assert_eq!(result.university_name, None);
    assert_eq!(result.course_name, None);
}
```

If `StoredResult` names those fields differently, use the actual field names (check `crates/core/src/domain/results.rs`); the assertion is "the result survives and its university/course links are empty".

Run: `cargo test -p academic-infra --test db_migrations` → Expected: FAIL (0002 not applied).

- [ ] **Step 3: Generalize the migration runner** in `crates/infra/src/db/mod.rs`:

```rust
/// Every migration, in order. Embedded at compile time so the binary never depends
/// on files next to it at runtime. Versions are never edited once released.
const MIGRATIONS: [(i64, &str); 2] = [
    (1, include_str!("../../migrations/0001_init.sql")),
    (2, include_str!("../../migrations/0002_lms.sql")),
];

fn apply_migrations(conn: &Connection) -> Result<(), RepoError> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_migrations (\
            version INTEGER PRIMARY KEY, applied_at TEXT NOT NULL);",
    )
    .map_err(map_err)?;

    for (version, sql) in MIGRATIONS {
        let applied: bool = conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM schema_migrations WHERE version = ?1)",
                [version],
                |row| row.get(0),
            )
            .map_err(map_err)?;
        if applied {
            continue;
        }
        // `unchecked_transaction` is safe here: `from_connection` runs before the
        // connection is shared behind its `Mutex`.
        let tx = conn.unchecked_transaction().map_err(map_err)?;
        tx.execute_batch(sql).map_err(map_err)?;
        tx.execute(
            "INSERT INTO schema_migrations (version, applied_at) VALUES (?1, ?2)",
            rusqlite::params![version, Utc::now().to_rfc3339()],
        )
        .map_err(map_err)?;
        tx.commit().map_err(map_err)?;
    }
    Ok(())
}
```

Keep the doc comment above `apply_migrations` (update it to say "each pending migration … in its own transaction"). Add `mod courses; mod universities;` to the module list.

- [ ] **Step 4: Reduce `CatalogRepository`** in `crates/core/src/ports/repositories.rs` to:

```rust
/// Read-only projection of universities and tracked courses, used by the email
/// analyzer as its course catalog (spec §11).
pub trait CatalogRepository: Send + Sync {
    fn catalog(&self) -> RepoResult<Catalog>;
}
```

Remove now-unused imports (`Course`, `University` if unused). In `crates/core/tests/support/store.rs`, delete the CRUD methods of `impl CatalogRepository for TestStore` and any fields/counters only they used (e.g. `next_university_id`, `next_course_id`), keeping `catalog()` and `fail_catalog_load`.

- [ ] **Step 5: Rewrite `crates/infra/src/db/catalog.rs`** to only implement `catalog()`:

```rust
//! `CatalogRepository` implementation: the analyzer's read-only view of the LMS
//! universities and their tracked, available courses.

use academic_core::domain::{Catalog, Course, University};
use academic_core::ports::{CatalogRepository, RepoResult};

use super::util::map_err;
use super::Database;

impl CatalogRepository for Database {
    fn catalog(&self) -> RepoResult<Catalog> {
        let conn = self.conn()?;

        let mut uni_stmt = conn
            .prepare("SELECT id, name FROM universities ORDER BY name")
            .map_err(map_err)?;
        let universities = uni_stmt
            .query_map([], |row| Ok(University { id: row.get(0)?, name: row.get(1)? }))
            .map_err(map_err)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(map_err)?;

        let mut course_stmt = conn
            .prepare(
                "SELECT id, university_id, name, short_name FROM courses \
                 WHERE tracked = 1 AND available = 1 ORDER BY university_id, name",
            )
            .map_err(map_err)?;
        let courses = course_stmt
            .query_map([], |row| {
                let name: String = row.get(2)?;
                let short_name: Option<String> = row.get(3)?;
                Ok(Course {
                    id: row.get(0)?,
                    university_id: row.get(1)?,
                    aliases: aliases_for(&name, short_name),
                    name,
                })
            })
            .map_err(map_err)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(map_err)?;

        Ok(Catalog { universities, courses })
    }
}

/// The short name is the course's only alias, when it adds information.
fn aliases_for(name: &str, short_name: Option<String>) -> Vec<String> {
    short_name
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty() && s != name)
        .into_iter()
        .collect()
}
```

- [ ] **Step 6: Write failing repository tests** — `crates/infra/tests/db_lms.rs`:

```rust
//! `UniversityRepository`, `CourseRepository` and the catalog projection.

use academic_core::domain::{LmsPlatform, LoginState, RemoteCourse, SyncOutcome};
use academic_core::ports::{CatalogRepository, CourseRepository, RepoError, UniversityRepository};
use academic_infra::db::Database;
use chrono::Utc;

fn remote(id: &str, name: &str, short: Option<&str>) -> RemoteCourse {
    RemoteCourse {
        external_id: id.into(),
        name: name.into(),
        short_name: short.map(str::to_string),
        url: Some(format!("https://x.es/course/{id}")),
    }
}

fn uv(db: &Database) -> i64 {
    db.insert_university("UV", LmsPlatform::Moodle, "https://aulavirtual.uv.es", Utc::now())
        .unwrap()
        .id
}

#[test]
fn inserted_university_starts_never_signed_in() {
    let db = Database::open_in_memory().unwrap();
    let u = db.insert_university("UV", LmsPlatform::Moodle, "https://aulavirtual.uv.es", Utc::now()).unwrap();
    assert_eq!(u.login_state, LoginState::Never);
    assert_eq!(u.account_label, None);
    assert_eq!(db.list_universities().unwrap(), vec![u.clone()]);
    assert_eq!(db.get_university(u.id).unwrap(), Some(u));
}

#[test]
fn duplicate_name_or_url_is_a_conflict() {
    let db = Database::open_in_memory().unwrap();
    uv(&db);
    let same_name = db.insert_university("UV", LmsPlatform::Blackboard, "https://other.es", Utc::now());
    assert!(matches!(same_name, Err(RepoError::Conflict(_))));
    let same_url = db.insert_university("Valencia", LmsPlatform::Moodle, "https://aulavirtual.uv.es", Utc::now());
    assert!(matches!(same_url, Err(RepoError::Conflict(_))));
}

#[test]
fn set_login_and_record_sync_update_the_row() {
    let db = Database::open_in_memory().unwrap();
    let id = uv(&db);
    let u = db.set_login(id, LoginState::Ok, Some("Ana Pérez")).unwrap();
    assert_eq!((u.login_state, u.account_label.as_deref()), (LoginState::Ok, Some("Ana Pérez")));
    let at = Utc::now();
    db.record_sync(id, at, SyncOutcome::Failed, Some("offline")).unwrap();
    let u = db.get_university(id).unwrap().unwrap();
    assert_eq!(u.last_sync_outcome, Some(SyncOutcome::Failed));
    assert_eq!(u.last_sync_error.as_deref(), Some("offline"));
    assert!(u.last_sync_at.is_some());
}

#[test]
fn missing_university_is_not_found() {
    let db = Database::open_in_memory().unwrap();
    assert!(matches!(db.rename_university(9, "x"), Err(RepoError::NotFound)));
    assert!(matches!(db.delete_university(9), Err(RepoError::NotFound)));
    assert!(matches!(db.set_login(9, LoginState::Ok, None), Err(RepoError::NotFound)));
    assert!(matches!(db.upsert_remote_courses(9, &[remote("1", "A", None)], Utc::now()), Err(RepoError::NotFound)));
}

#[test]
fn upsert_inserts_updates_and_flags_missing_courses() {
    let db = Database::open_in_memory().unwrap();
    let id = uv(&db);
    db.upsert_remote_courses(id, &[remote("1", "Algebra", Some("ALG")), remote("2", "Calculus", None)], Utc::now()).unwrap();
    let courses = db.list_courses(id).unwrap();
    assert_eq!(courses.len(), 2);
    assert!(courses.iter().all(|c| !c.tracked && !c.reviewed && c.available));

    db.upsert_remote_courses(id, &[remote("1", "Algebra II", Some("ALG2"))], Utc::now()).unwrap();
    let courses = db.list_courses(id).unwrap();
    let algebra = courses.iter().find(|c| c.external_id == "1").unwrap();
    assert_eq!((algebra.name.as_str(), algebra.short_name.as_deref()), ("Algebra II", Some("ALG2")));
    assert!(algebra.available);
    assert!(!courses.iter().find(|c| c.external_id == "2").unwrap().available);

    db.upsert_remote_courses(id, &[remote("1", "Algebra II", None), remote("2", "Calculus", None)], Utc::now()).unwrap();
    assert!(db.list_courses(id).unwrap().iter().all(|c| c.available));
}

#[test]
fn set_tracked_courses_tracks_exactly_the_given_ids_and_reviews_all() {
    let db = Database::open_in_memory().unwrap();
    let id = uv(&db);
    db.upsert_remote_courses(id, &[remote("1", "Algebra", None), remote("2", "Calculus", None)], Utc::now()).unwrap();
    let ids: Vec<i64> = db.list_courses(id).unwrap().iter().map(|c| c.id).collect();

    let courses = db.set_tracked_courses(id, &[ids[0]]).unwrap();
    assert_eq!(courses.iter().filter(|c| c.tracked).count(), 1);
    assert!(courses.iter().all(|c| c.reviewed));

    let other = db.insert_university("UAH", LmsPlatform::Blackboard, "https://uah.blackboard.com", Utc::now()).unwrap();
    assert!(matches!(db.set_tracked_courses(other.id, &[ids[1]]), Err(RepoError::NotFound)));
    assert_eq!(db.list_courses(id).unwrap().iter().filter(|c| c.tracked).count(), 1, "no partial change");
}

#[test]
fn catalog_lists_tracked_available_courses_with_short_name_alias() {
    let db = Database::open_in_memory().unwrap();
    let id = uv(&db);
    db.upsert_remote_courses(
        id,
        &[remote("1", "Algebra", Some("ALG")), remote("2", "Calculus", Some("Calculus")), remote("3", "Physics", None)],
        Utc::now(),
    )
    .unwrap();
    let ids: Vec<i64> = db.list_courses(id).unwrap().iter().map(|c| c.id).collect();
    db.set_tracked_courses(id, &ids[..2]).unwrap();

    let catalog = db.catalog().unwrap();
    assert_eq!(catalog.universities.len(), 1);
    assert_eq!(catalog.courses.len(), 2);
    let algebra = catalog.courses.iter().find(|c| c.name == "Algebra").unwrap();
    assert_eq!(algebra.aliases, vec!["ALG"]);
    let calculus = catalog.courses.iter().find(|c| c.name == "Calculus").unwrap();
    assert!(calculus.aliases.is_empty(), "same as the name adds nothing");
}

#[test]
fn deleting_a_university_removes_its_courses() {
    let db = Database::open_in_memory().unwrap();
    let id = uv(&db);
    db.upsert_remote_courses(id, &[remote("1", "Algebra", None)], Utc::now()).unwrap();
    db.delete_university(id).unwrap();
    assert!(db.list_universities().unwrap().is_empty());
    assert!(db.list_courses(id).unwrap().is_empty());
}
```

`list_courses` is ordered by name (`ORDER BY name`), so `ids[0]` is "Algebra".

Run: `cargo test -p academic-infra --test db_lms` → Expected: compile errors (impls missing).

- [ ] **Step 7: Implement `db/universities.rs`**. Follow the structure of `db/accounts.rs` (raw row struct read inside `query_map`, conversion with `parse_enum`/`parse_datetime(_opt)` afterwards, `map_err` for every rusqlite error, `RepoError::NotFound` when an `UPDATE`/`DELETE` changes 0 rows):

```rust
//! `UniversityRepository` implementation.

use academic_core::domain::{LmsPlatform, LmsUniversity, LoginState, SyncOutcome, UniversityId};
use academic_core::ports::{RepoError, RepoResult, UniversityRepository};
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension};

use super::util::{map_err, parse_datetime, parse_datetime_opt, parse_enum, to_rfc3339};
use super::Database;

const SELECT: &str = "SELECT id, name, platform, base_url, login_state, account_label, \
    last_sync_at, last_sync_outcome, last_sync_error, created_at FROM universities";

struct Raw {
    id: i64,
    name: String,
    platform: String,
    base_url: String,
    login_state: String,
    account_label: Option<String>,
    last_sync_at: Option<String>,
    last_sync_outcome: Option<String>,
    last_sync_error: Option<String>,
    created_at: String,
}

fn read_raw(row: &rusqlite::Row) -> rusqlite::Result<Raw> {
    Ok(Raw {
        id: row.get(0)?,
        name: row.get(1)?,
        platform: row.get(2)?,
        base_url: row.get(3)?,
        login_state: row.get(4)?,
        account_label: row.get(5)?,
        last_sync_at: row.get(6)?,
        last_sync_outcome: row.get(7)?,
        last_sync_error: row.get(8)?,
        created_at: row.get(9)?,
    })
}

fn convert(raw: Raw) -> RepoResult<LmsUniversity> {
    Ok(LmsUniversity {
        id: raw.id,
        name: raw.name,
        platform: parse_enum::<LmsPlatform>(&raw.platform)?,
        base_url: raw.base_url,
        login_state: parse_enum::<LoginState>(&raw.login_state)?,
        account_label: raw.account_label,
        last_sync_at: parse_datetime_opt(raw.last_sync_at.as_deref())?,
        last_sync_outcome: raw
            .last_sync_outcome
            .as_deref()
            .map(parse_enum::<SyncOutcome>)
            .transpose()?,
        last_sync_error: raw.last_sync_error,
        created_at: parse_datetime(&raw.created_at)?,
    })
}

fn get_on(conn: &Connection, id: UniversityId) -> RepoResult<Option<LmsUniversity>> {
    conn.query_row(&format!("{SELECT} WHERE id = ?1"), [id], read_raw)
        .optional()
        .map_err(map_err)?
        .map(convert)
        .transpose()
}

fn require(conn: &Connection, id: UniversityId) -> RepoResult<LmsUniversity> {
    get_on(conn, id)?.ok_or(RepoError::NotFound)
}

fn ensure_changed(changed: usize) -> RepoResult<()> {
    if changed == 0 { Err(RepoError::NotFound) } else { Ok(()) }
}

impl UniversityRepository for Database {
    fn list_universities(&self) -> RepoResult<Vec<LmsUniversity>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(&format!("{SELECT} ORDER BY name")).map_err(map_err)?;
        let raws = stmt
            .query_map([], read_raw)
            .map_err(map_err)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(map_err)?;
        raws.into_iter().map(convert).collect()
    }

    fn get_university(&self, id: UniversityId) -> RepoResult<Option<LmsUniversity>> {
        get_on(&self.conn()?, id)
    }

    fn insert_university(
        &self,
        name: &str,
        platform: LmsPlatform,
        base_url: &str,
        now: DateTime<Utc>,
    ) -> RepoResult<LmsUniversity> {
        let conn = self.conn()?;
        conn.execute(
            "INSERT INTO universities (name, platform, base_url, created_at) VALUES (?1, ?2, ?3, ?4)",
            params![name, platform.as_str(), base_url, to_rfc3339(now)],
        )
        .map_err(map_err)?;
        require(&conn, conn.last_insert_rowid())
    }

    fn rename_university(&self, id: UniversityId, name: &str) -> RepoResult<LmsUniversity> {
        let conn = self.conn()?;
        ensure_changed(
            conn.execute("UPDATE universities SET name = ?1 WHERE id = ?2", params![name, id])
                .map_err(map_err)?,
        )?;
        require(&conn, id)
    }

    fn delete_university(&self, id: UniversityId) -> RepoResult<()> {
        let conn = self.conn()?;
        ensure_changed(
            conn.execute("DELETE FROM universities WHERE id = ?1", [id]).map_err(map_err)?,
        )
    }

    fn set_login(
        &self,
        id: UniversityId,
        state: LoginState,
        account_label: Option<&str>,
    ) -> RepoResult<LmsUniversity> {
        let conn = self.conn()?;
        ensure_changed(
            conn.execute(
                "UPDATE universities SET login_state = ?1, account_label = ?2 WHERE id = ?3",
                params![state.as_str(), account_label, id],
            )
            .map_err(map_err)?,
        )?;
        require(&conn, id)
    }

    fn record_sync(
        &self,
        id: UniversityId,
        at: DateTime<Utc>,
        outcome: SyncOutcome,
        error: Option<&str>,
    ) -> RepoResult<()> {
        let conn = self.conn()?;
        ensure_changed(
            conn.execute(
                "UPDATE universities SET last_sync_at = ?1, last_sync_outcome = ?2, \
                 last_sync_error = ?3 WHERE id = ?4",
                params![to_rfc3339(at), outcome.as_str(), error, id],
            )
            .map_err(map_err)?,
        )
    }
}
```

Adjust helper signatures to the real ones in `db/util.rs` (`parse_datetime_opt` takes `Option<&str>` or `Option<String>` — check and adapt). If `db/util.rs` lacks a generic "0 rows → NotFound" helper, keep `ensure_changed` local as above.

- [ ] **Step 8: Implement `db/courses.rs`** with the same conventions:

```rust
//! `CourseRepository` implementation.

use academic_core::domain::{LmsCourse, RemoteCourse, UniversityId};
use academic_core::ports::{CourseRepository, RepoError, RepoResult};
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};

use super::util::{map_err, parse_datetime_opt, to_rfc3339};
use super::Database;

fn query_courses(conn: &Connection, university_id: UniversityId) -> RepoResult<Vec<LmsCourse>> {
    let mut stmt = conn
        .prepare(
            "SELECT id, university_id, external_id, name, short_name, url, tracked, available, \
             reviewed, items_synced_at FROM courses WHERE university_id = ?1 ORDER BY name, id",
        )
        .map_err(map_err)?;
    let rows = stmt
        .query_map([university_id], |row| {
            Ok((
                LmsCourse {
                    id: row.get(0)?,
                    university_id: row.get(1)?,
                    external_id: row.get(2)?,
                    name: row.get(3)?,
                    short_name: row.get(4)?,
                    url: row.get(5)?,
                    tracked: row.get(6)?,
                    available: row.get(7)?,
                    reviewed: row.get(8)?,
                    items_synced_at: None,
                },
                row.get::<_, Option<String>>(9)?,
            ))
        })
        .map_err(map_err)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(map_err)?;
    rows.into_iter()
        .map(|(mut course, synced)| {
            course.items_synced_at = parse_datetime_opt(synced.as_deref())?;
            Ok(course)
        })
        .collect()
}

fn university_exists(conn: &Connection, id: UniversityId) -> RepoResult<bool> {
    conn.query_row("SELECT EXISTS(SELECT 1 FROM universities WHERE id = ?1)", [id], |r| r.get(0))
        .map_err(map_err)
}

impl CourseRepository for Database {
    fn list_courses(&self, university_id: UniversityId) -> RepoResult<Vec<LmsCourse>> {
        query_courses(&self.conn()?, university_id)
    }

    fn upsert_remote_courses(
        &self,
        university_id: UniversityId,
        courses: &[RemoteCourse],
        now: DateTime<Utc>,
    ) -> RepoResult<()> {
        let mut conn = self.conn()?;
        let tx = conn.transaction().map_err(map_err)?;
        if !university_exists(&tx, university_id)? {
            return Err(RepoError::NotFound);
        }
        tx.execute("UPDATE courses SET available = 0 WHERE university_id = ?1", [university_id])
            .map_err(map_err)?;
        for course in courses {
            tx.execute(
                "INSERT INTO courses (university_id, external_id, name, short_name, url, created_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6) \
                 ON CONFLICT (university_id, external_id) DO UPDATE SET \
                 name = excluded.name, short_name = excluded.short_name, url = excluded.url, available = 1",
                params![
                    university_id,
                    course.external_id,
                    course.name,
                    course.short_name,
                    course.url,
                    to_rfc3339(now)
                ],
            )
            .map_err(map_err)?;
        }
        tx.commit().map_err(map_err)
    }

    fn set_tracked_courses(
        &self,
        university_id: UniversityId,
        course_ids: &[i64],
    ) -> RepoResult<Vec<LmsCourse>> {
        let mut conn = self.conn()?;
        let tx = conn.transaction().map_err(map_err)?;
        tx.execute(
            "UPDATE courses SET tracked = 0, reviewed = 1 WHERE university_id = ?1",
            [university_id],
        )
        .map_err(map_err)?;
        for id in course_ids {
            let changed = tx
                .execute(
                    "UPDATE courses SET tracked = 1 WHERE id = ?1 AND university_id = ?2",
                    params![id, university_id],
                )
                .map_err(map_err)?;
            if changed == 0 {
                return Err(RepoError::NotFound); // dropping `tx` rolls back
            }
        }
        tx.commit().map_err(map_err)?;
        query_courses(&conn, university_id)
    }
}
```

The test `missing_university_is_not_found` expects `NotFound` from `upsert_remote_courses` on a missing university even with courses in the input: the `university_exists` check provides that before any FK error.

- [ ] **Step 9: Fix the remaining callers**
  - `crates/infra/tests/db_results.rs` (~line 413): replace `db.add_university("UV", …)` / `db.add_course(…)` with `db.insert_university("UV", LmsPlatform::Moodle, "https://aulavirtual.uv.es", Utc::now())`, then `db.upsert_remote_courses(uni.id, &[RemoteCourse { external_id: "1".into(), name: "Programacion".into(), short_name: None, url: None }], Utc::now())` and take the id from `db.list_courses(uni.id)`. Keep the test's assertions unchanged. Fix its imports (drop `CatalogRepository` if unused, add `UniversityRepository`, `CourseRepository`).
  - Delete `crates/infra/tests/db_catalog.rs` (its behavior no longer exists; the catalog projection is covered in `db_lms.rs`).
  - `src-tauri/src/commands/catalog.rs`: keep `get_catalog` and the private `catalog()` helper; delete the other commands, helpers and tests; update the module doc to "Command: read-only catalog (universities and tracked courses) for the email screens". Add one test:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::harness;

    #[test]
    fn catalog_is_empty_without_universities() {
        let h = harness();
        assert!(catalog(&h.state).unwrap().is_empty());
    }
}
```

  - `src-tauri/src/lib.rs`: remove `catalog::add_university`, `update_university`, `remove_university`, `add_course`, `update_course`, `remove_course` from `generate_handler!`.
  - `src-tauri/src/dto/catalog.rs`: `UniversityDto::new` may become unused outside tests; if clippy reports dead code, make it private to the module (`fn new`) rather than deleting the type.

- [ ] **Step 10: Run all tests and gates**

Run: `cargo test --workspace` → Expected: PASS. Then the Rust gates.

- [ ] **Step 11: Commit**

```bash
git add -A crates src-tauri
git commit -m "feat(db): add LMS universities and courses, retire the manual catalog"
```

---

### Task 3: Platform probe and Moodle/Blackboard clients (`verify`, `list_courses`)

**Files:**
- Modify: `Cargo.toml` (workspace deps: add `md-5`), `crates/infra/Cargo.toml` (add `md-5.workspace = true`, `hex.workspace = true`)
- Modify: `crates/infra/src/http_util.rs` (move `encode_form` here, `pub(crate)`, with its unit test), `crates/infra/src/graph/oauth.rs` (use `crate::http_util::encode_form`, delete the local copy and its test)
- Create: `crates/infra/src/lms/mod.rs`, `lms/moodle.rs`, `lms/blackboard.rs`, `lms/probe.rs`, `lms/factory.rs`
- Modify: `crates/infra/src/lib.rs` (`pub mod lms;`, update crate doc)
- Modify: `crates/infra/src/secrets.rs` (add `pub const MAX_SECRET_BYTES: usize = 2560;` with a doc comment: Windows Credential Manager's `CRED_MAX_CREDENTIAL_BLOB_SIZE`)
- Test: `crates/infra/tests/lms_moodle.rs`, `lms_blackboard.rs`, `lms_probe.rs`, `lms_factory.rs`

**Interfaces:**
- Consumes (Task 1): `LmsClient`, `LmsClientFactory`, `LmsError`, `LmsIdentity`, `RemoteCourse`, `LmsUniversity`, `LmsPlatform`, `lms_credential_key`, `SecretStore`.
- Produces:
  - `academic_infra::lms::moodle::{MoodleClient, decode_launch_token}` — `MoodleClient::new(http: reqwest::Client, base_url: String, token: String)`; `decode_launch_token(payload: &str, base_url: &str, passport: &str) -> Result<String, LmsError>`
  - `academic_infra::lms::blackboard::{BlackboardClient, SessionCookie, encode_session, decode_session}` — `BlackboardClient::new(http, base_url: String, cookies: &[SessionCookie])`; `SessionCookie { pub name: String, pub value: String }` (serde); `encode_session(&[SessionCookie]) -> String`; `decode_session(&str) -> Result<Vec<SessionCookie>, LmsError>` (unparseable or empty → `Auth`)
  - `academic_infra::lms::probe::{probe, ProbeError}` — `async fn probe(http: &reqwest::Client, platform: LmsPlatform, base_url: &str) -> Result<(), ProbeError>`; `ProbeError::{Mismatch(String), Connection(String)}` (Display = the message)
  - `academic_infra::lms::factory::DefaultLmsClientFactory::new(secrets: Arc<dyn SecretStore>, http: reqwest::Client)`
  - `academic_infra::secrets::MAX_SECRET_BYTES`

- [ ] **Step 1: Dependencies.** Add the newest `md-5` release to `[workspace.dependencies]` (`cargo search md-5 --limit 1` to get the version; it must build together with `sha2 = 0.11` — if the newest `md-5` pulls a different `digest` major that conflicts, pick the `md-5` version that uses the same `digest` as `sha2 0.11` and say so in your report). Add `md-5.workspace = true` and `hex.workspace = true` to `crates/infra/Cargo.toml`.

- [ ] **Step 2: Move `encode_form`** from `graph/oauth.rs` into `http_util.rs` as `pub(crate) fn encode_form(pairs: &[(&str, &str)]) -> String`, moving its unit test too; update the module doc of `http_util.rs` to "Shared HTTP helpers for the infra adapters". Run `cargo test -p academic-infra` → PASS.

- [ ] **Step 3: Shared LMS helpers** — `crates/infra/src/lms/mod.rs`:

```rust
//! LMS adapters (spec §5): the public platform probe, Moodle and Blackboard
//! clients, and the factory that builds them from stored credentials.

pub mod blackboard;
pub mod factory;
pub mod moodle;
pub mod probe;

use std::time::Duration;

use academic_core::ports::LmsError;

use crate::http_util::describe_transport_error;

/// Per-request timeout for every LMS call.
pub(crate) const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

pub(crate) fn connection_error(error: &reqwest::Error) -> LmsError {
    LmsError::Connection(format!("Could not reach the platform: {}", describe_transport_error(error)))
}
```

`describe_transport_error` mentions "the provider" for connect errors; that wording is acceptable here (do not change it).

- [ ] **Step 4: Write failing Moodle tests** — `crates/infra/tests/lms_moodle.rs`:

```rust
use academic_core::ports::{LmsClient, LmsError};
use academic_infra::lms::moodle::{decode_launch_token, MoodleClient};
use base64::Engine;
use md5::{Digest, Md5};
use serde_json::json;
use wiremock::matchers::{body_string_contains, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const REQUIRED: [&str; 7] = [
    "core_enrol_get_users_courses",
    "mod_assign_get_assignments",
    "mod_assign_get_submission_status",
    "mod_quiz_get_quizzes_by_courses",
    "mod_quiz_get_user_attempts",
    "mod_quiz_get_user_best_grade",
    "core_calendar_get_calendar_events",
];

fn site_info(functions: &[&str]) -> serde_json::Value {
    json!({
        "userid": 42,
        "fullname": "Ana Pérez",
        "functions": functions.iter().map(|f| json!({ "name": f, "version": "1" })).collect::<Vec<_>>()
    })
}

async fn mock_site_info(server: &MockServer, body: serde_json::Value) {
    Mock::given(method("POST"))
        .and(path("/webservice/rest/server.php"))
        .and(body_string_contains("wsfunction=core_webservice_get_site_info"))
        .respond_with(ResponseTemplate::new(200).set_body_json(body))
        .mount(server)
        .await;
}

fn client(server: &MockServer) -> MoodleClient {
    MoodleClient::new(reqwest::Client::new(), server.uri(), "tok123".into())
}

#[tokio::test]
async fn verify_returns_the_user_and_sends_the_token() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/webservice/rest/server.php"))
        .and(body_string_contains("wstoken=tok123"))
        .and(body_string_contains("moodlewsrestformat=json"))
        .and(body_string_contains("wsfunction=core_webservice_get_site_info"))
        .respond_with(ResponseTemplate::new(200).set_body_json(site_info(&REQUIRED)))
        .mount(&server)
        .await;
    let identity = client(&server).verify().await.unwrap();
    assert_eq!((identity.user_id.as_str(), identity.display_name.as_str()), ("42", "Ana Pérez"));
}

#[tokio::test]
async fn verify_fails_when_a_required_function_is_missing() {
    let server = MockServer::start().await;
    mock_site_info(&server, site_info(&REQUIRED[..6])).await;
    match client(&server).verify().await {
        Err(LmsError::Protocol(m)) => assert!(m.contains("core_calendar_get_calendar_events"), "{m}"),
        other => panic!("{other:?}"),
    }
}

#[tokio::test]
async fn invalid_token_is_an_auth_error() {
    let server = MockServer::start().await;
    mock_site_info(
        &server,
        json!({ "exception": "moodle_exception", "errorcode": "invalidtoken", "message": "Invalid token" }),
    )
    .await;
    assert!(matches!(client(&server).verify().await, Err(LmsError::Auth(_))));
}

#[tokio::test]
async fn unavailable_function_is_a_protocol_error_naming_it() {
    let server = MockServer::start().await;
    mock_site_info(
        &server,
        json!({ "exception": "x", "errorcode": "servicenotavailable", "message": "nope" }),
    )
    .await;
    match client(&server).verify().await {
        Err(LmsError::Protocol(m)) => assert!(m.contains("core_webservice_get_site_info")),
        other => panic!("{other:?}"),
    }
}

#[tokio::test]
async fn list_courses_maps_enrolled_courses() {
    let server = MockServer::start().await;
    mock_site_info(&server, site_info(&REQUIRED)).await;
    Mock::given(method("POST"))
        .and(path("/webservice/rest/server.php"))
        .and(body_string_contains("wsfunction=core_enrol_get_users_courses"))
        .and(body_string_contains("userid=42"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([
            { "id": 1234, "fullname": "Cálculo II", "shortname": "CAL2" },
            { "id": 99, "fullname": "Tutoría", "shortname": "" }
        ])))
        .mount(&server)
        .await;
    let courses = client(&server).list_courses().await.unwrap();
    assert_eq!(courses.len(), 2);
    assert_eq!(courses[0].external_id, "1234");
    assert_eq!(courses[0].name, "Cálculo II");
    assert_eq!(courses[0].short_name.as_deref(), Some("CAL2"));
    assert_eq!(courses[0].url, Some(format!("{}/course/view.php?id=1234", server.uri())));
    assert_eq!(courses[1].short_name, None);
}

#[tokio::test]
async fn http_errors_are_protocol_errors() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(503))
        .mount(&server)
        .await;
    assert!(matches!(client(&server).verify().await, Err(LmsError::Protocol(_))));
}

fn payload(signature: &str, token: &str) -> String {
    base64::engine::general_purpose::STANDARD.encode(format!("{signature}:::{token}:::private"))
}

fn signature(base_url: &str, passport: &str) -> String {
    hex::encode(Md5::digest(format!("{base_url}{passport}").as_bytes()))
}

#[test]
fn launch_token_is_decoded_when_the_signature_matches() {
    let base = "https://aulavirtual.uv.es";
    let p = payload(&signature(base, "pass1"), "tok-xyz");
    assert_eq!(decode_launch_token(&p, base, "pass1").unwrap(), "tok-xyz");
}

#[test]
fn launch_token_accepts_percent_encoded_base64() {
    let base = "https://aulavirtual.uv.es";
    let p = payload(&signature(base, "pass1"), "tok-xyz")
        .replace('+', "%2B").replace('/', "%2F").replace('=', "%3D");
    assert_eq!(decode_launch_token(&p, base, "pass1").unwrap(), "tok-xyz");
}

#[test]
fn launch_token_with_a_wrong_signature_or_garbage_is_rejected() {
    let base = "https://aulavirtual.uv.es";
    let wrong = payload(&signature(base, "other"), "tok-xyz");
    assert!(matches!(decode_launch_token(&wrong, base, "pass1"), Err(LmsError::Auth(_))));
    assert!(matches!(decode_launch_token("%%%", base, "pass1"), Err(LmsError::Auth(_))));
    let no_token = base64::engine::general_purpose::STANDARD.encode("onlysignature");
    assert!(matches!(decode_launch_token(&no_token, base, "pass1"), Err(LmsError::Auth(_))));
}
```

Add `md-5`, `hex`, `base64` to `[dev-dependencies]` of `crates/infra` only if integration tests cannot already use them (regular dependencies are visible to integration tests, so normally nothing is needed). The `md5` import path is the `md-5` crate's library name (`md5`).

Run: `cargo test -p academic-infra --test lms_moodle` → Expected: compile errors.

- [ ] **Step 5: Implement `lms/moodle.rs`**:

```rust
//! Moodle web-service client (mobile service token) and launch-token decoding (spec §5.1, §6.2).

use academic_core::domain::{LmsIdentity, RemoteCourse};
use academic_core::ports::{LmsClient, LmsError};
use async_trait::async_trait;
use base64::Engine;
use md5::{Digest, Md5};
use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde_json::Value;

use super::{connection_error, REQUEST_TIMEOUT};
use crate::http_util::encode_form;

/// Functions Phase 2 needs from the mobile service. `verify` fails when one is missing.
const REQUIRED_FUNCTIONS: [&str; 7] = [
    "core_enrol_get_users_courses",
    "mod_assign_get_assignments",
    "mod_assign_get_submission_status",
    "mod_quiz_get_quizzes_by_courses",
    "mod_quiz_get_user_attempts",
    "mod_quiz_get_user_best_grade",
    "core_calendar_get_calendar_events",
];

pub struct MoodleClient {
    http: reqwest::Client,
    base_url: String,
    token: String,
}

#[derive(Deserialize)]
struct SiteInfo {
    userid: i64,
    fullname: String,
    #[serde(default)]
    functions: Vec<SiteFunction>,
}

#[derive(Deserialize)]
struct SiteFunction {
    name: String,
}

#[derive(Deserialize)]
struct MoodleCourse {
    id: i64,
    fullname: String,
    #[serde(default)]
    shortname: String,
}

impl MoodleClient {
    pub fn new(http: reqwest::Client, base_url: String, token: String) -> Self {
        Self { http, base_url, token }
    }

    async fn call<T: DeserializeOwned>(
        &self,
        function: &str,
        args: &[(&str, &str)],
    ) -> Result<T, LmsError> {
        let mut form: Vec<(&str, &str)> = vec![
            ("wstoken", self.token.as_str()),
            ("wsfunction", function),
            ("moodlewsrestformat", "json"),
        ];
        form.extend_from_slice(args);
        let response = self
            .http
            .post(format!("{}/webservice/rest/server.php", self.base_url))
            .header(reqwest::header::CONTENT_TYPE, "application/x-www-form-urlencoded")
            .body(encode_form(&form))
            .timeout(REQUEST_TIMEOUT)
            .send()
            .await
            .map_err(|e| connection_error(&e))?;
        let status = response.status();
        if !status.is_success() {
            return Err(LmsError::Protocol(format!("{function} returned HTTP {}", status.as_u16())));
        }
        let value: Value = response
            .json()
            .await
            .map_err(|_| LmsError::Protocol(format!("{function} returned an unexpected response")))?;
        if let Some(code) = value.get("errorcode").and_then(Value::as_str) {
            return Err(map_error_code(function, code));
        }
        serde_json::from_value(value)
            .map_err(|_| LmsError::Protocol(format!("{function} returned an unexpected response")))
    }

    async fn site_info(&self) -> Result<SiteInfo, LmsError> {
        self.call("core_webservice_get_site_info", &[]).await
    }
}

fn map_error_code(function: &str, code: &str) -> LmsError {
    match code {
        "invalidtoken" => LmsError::Auth("Your Moodle session has expired".into()),
        "servicenotavailable" | "accessexception" => {
            LmsError::Protocol(format!("{function} is not available on this Moodle site"))
        }
        other => LmsError::Protocol(format!("{function} failed ({other})")),
    }
}

#[async_trait]
impl LmsClient for MoodleClient {
    async fn verify(&self) -> Result<LmsIdentity, LmsError> {
        let info = self.site_info().await?;
        if let Some(missing) = REQUIRED_FUNCTIONS
            .iter()
            .find(|required| !info.functions.iter().any(|f| f.name == **required))
        {
            return Err(LmsError::Protocol(format!(
                "{missing} is not enabled for the Moodle mobile service on this site"
            )));
        }
        Ok(LmsIdentity { user_id: info.userid.to_string(), display_name: info.fullname })
    }

    async fn list_courses(&self) -> Result<Vec<RemoteCourse>, LmsError> {
        let user_id = self.site_info().await?.userid.to_string();
        let courses: Vec<MoodleCourse> = self
            .call("core_enrol_get_users_courses", &[("userid", user_id.as_str())])
            .await?;
        Ok(courses
            .into_iter()
            .map(|c| RemoteCourse {
                external_id: c.id.to_string(),
                url: Some(format!("{}/course/view.php?id={}", self.base_url, c.id)),
                short_name: Some(c.shortname.trim().to_string()).filter(|s| !s.is_empty()),
                name: c.fullname,
            })
            .collect())
    }
}

/// Decodes the payload of a `moodlemobile://token=<payload>` redirect. The payload is
/// base64 of `signature:::token[:::privatetoken]`; `signature` must equal
/// `md5(base_url + passport)`. Returns only the token (the private token is dropped).
pub fn decode_launch_token(payload: &str, base_url: &str, passport: &str) -> Result<String, LmsError> {
    let invalid = || LmsError::Auth("Moodle returned an invalid sign-in token".into());
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(percent_decode_base64(payload.trim()))
        .map_err(|_| invalid())?;
    let text = String::from_utf8(decoded).map_err(|_| invalid())?;
    let mut parts = text.split(":::");
    let (Some(signature), Some(token)) = (parts.next(), parts.next()) else {
        return Err(invalid());
    };
    if token.is_empty() {
        return Err(invalid());
    }
    let expected = hex::encode(Md5::digest(format!("{base_url}{passport}").as_bytes()));
    if !signature.eq_ignore_ascii_case(&expected) {
        return Err(LmsError::Auth("The Moodle sign-in could not be verified".into()));
    }
    Ok(token.to_string())
}

/// Undoes percent-encoding of the three base64 characters a webview may escape.
fn percent_decode_base64(value: &str) -> String {
    value
        .replace("%2B", "+").replace("%2b", "+")
        .replace("%2F", "/").replace("%2f", "/")
        .replace("%3D", "=").replace("%3d", "=")
}
```

Run: `cargo test -p academic-infra --test lms_moodle` → PASS.

- [ ] **Step 6: Write failing Blackboard tests** — `crates/infra/tests/lms_blackboard.rs`:

```rust
use academic_core::ports::{LmsClient, LmsError};
use academic_infra::lms::blackboard::{decode_session, encode_session, BlackboardClient, SessionCookie};
use serde_json::json;
use wiremock::matchers::{header, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn cookies() -> Vec<SessionCookie> {
    vec![
        SessionCookie { name: "BbRouter".into(), value: "expires:1,id:abc".into() },
        SessionCookie { name: "JSESSIONID".into(), value: "J1".into() },
    ]
}

fn client(server: &MockServer) -> BlackboardClient {
    BlackboardClient::new(reqwest::Client::new(), server.uri(), &cookies())
}

#[tokio::test]
async fn verify_sends_the_cookies_and_builds_the_display_name() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/learn/api/public/v1/users/me"))
        .and(header("cookie", "BbRouter=expires:1,id:abc; JSESSIONID=J1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "_1234_1", "userName": "ana.perez", "name": { "given": "Ana", "family": "Pérez" }
        })))
        .mount(&server)
        .await;
    let identity = client(&server).verify().await.unwrap();
    assert_eq!((identity.user_id.as_str(), identity.display_name.as_str()), ("_1234_1", "Ana Pérez"));
}

#[tokio::test]
async fn verify_falls_back_to_the_user_name() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/learn/api/public/v1/users/me"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "id": "_1_1", "userName": "ana.perez" })))
        .mount(&server)
        .await;
    assert_eq!(client(&server).verify().await.unwrap().display_name, "ana.perez");
}

#[tokio::test]
async fn unauthorized_is_an_auth_error() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(401).set_body_json(json!({ "status": 401 })))
        .mount(&server)
        .await;
    assert!(matches!(client(&server).verify().await, Err(LmsError::Auth(_))));
}

#[tokio::test]
async fn a_redirect_away_from_the_api_is_an_auth_error() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/learn/api/public/v1/users/me"))
        .respond_with(ResponseTemplate::new(302).insert_header("location", "/webapps/login/"))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/webapps/login/"))
        .respond_with(ResponseTemplate::new(200).set_body_string("<html>login</html>"))
        .mount(&server)
        .await;
    assert!(matches!(client(&server).verify().await, Err(LmsError::Auth(_))));
}

#[tokio::test]
async fn list_courses_follows_paging() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/learn/api/public/v1/users/me/courses"))
        .and(query_param("offset", "100"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "results": [ { "courseId": "_2_1", "course": { "id": "_2_1", "courseId": "RED-2026", "name": "Redes" } } ]
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/learn/api/public/v1/users/me/courses"))
        .and(query_param("expand", "course"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "results": [ { "courseId": "_10_1", "course": { "id": "_10_1", "courseId": "ISW-2026", "name": "Ingeniería del Software" } } ],
            "paging": { "nextPage": "/learn/api/public/v1/users/me/courses?expand=course&limit=100&offset=100" }
        })))
        .mount(&server)
        .await;
    let courses = client(&server).list_courses().await.unwrap();
    assert_eq!(courses.len(), 2);
    assert_eq!(courses[0].external_id, "_10_1");
    assert_eq!(courses[0].short_name.as_deref(), Some("ISW-2026"));
    assert_eq!(courses[0].url, Some(format!("{}/ultra/courses/_10_1/outline", server.uri())));
    assert_eq!(courses[1].name, "Redes");
}

#[test]
fn sessions_round_trip_and_garbage_is_an_auth_error() {
    let json = encode_session(&cookies());
    assert_eq!(decode_session(&json).unwrap(), cookies());
    assert!(matches!(decode_session("not json"), Err(LmsError::Auth(_))));
    assert!(matches!(decode_session("[]"), Err(LmsError::Auth(_))));
}
```

Mount order matters in wiremock: the more specific `offset=100` mock is mounted first; if matching is ambiguous, add `query_param_is_missing("offset")` to the first-page mock.

- [ ] **Step 7: Implement `lms/blackboard.rs`**:

```rust
//! Blackboard Learn public REST client using the user's session cookies (spec §5.2).

use academic_core::domain::{LmsIdentity, RemoteCourse};
use academic_core::ports::{LmsClient, LmsError};
use async_trait::async_trait;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use super::{connection_error, REQUEST_TIMEOUT};

/// Upper bound on followed `paging.nextPage` links, against a misbehaving server.
const MAX_PAGES: usize = 50;
const API_PREFIX: &str = "/learn/api/";

/// One captured session cookie. The stored credential is a JSON array of these.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionCookie {
    pub name: String,
    pub value: String,
}

pub fn encode_session(cookies: &[SessionCookie]) -> String {
    serde_json::to_string(cookies).expect("cookies always serialize")
}

/// An unreadable or empty session is treated as an expired one.
pub fn decode_session(json: &str) -> Result<Vec<SessionCookie>, LmsError> {
    serde_json::from_str::<Vec<SessionCookie>>(json)
        .ok()
        .filter(|cookies| !cookies.is_empty())
        .ok_or_else(|| LmsError::Auth("The saved Blackboard session is not usable".into()))
}

pub struct BlackboardClient {
    http: reqwest::Client,
    base_url: String,
    cookie_header: String,
}

#[derive(Deserialize)]
struct Me {
    id: String,
    #[serde(rename = "userName")]
    user_name: Option<String>,
    name: Option<PersonName>,
}

#[derive(Deserialize)]
struct PersonName {
    given: Option<String>,
    family: Option<String>,
}

#[derive(Deserialize)]
struct Page<T> {
    results: Vec<T>,
    paging: Option<Paging>,
}

#[derive(Deserialize)]
struct Paging {
    #[serde(rename = "nextPage")]
    next_page: Option<String>,
}

#[derive(Deserialize)]
struct Membership {
    #[serde(rename = "courseId")]
    course_id: String,
    course: Option<CourseInfo>,
}

#[derive(Deserialize)]
struct CourseInfo {
    #[serde(rename = "courseId")]
    code: Option<String>,
    name: String,
}

impl BlackboardClient {
    pub fn new(http: reqwest::Client, base_url: String, cookies: &[SessionCookie]) -> Self {
        let cookie_header = cookies
            .iter()
            .map(|c| format!("{}={}", c.name, c.value))
            .collect::<Vec<_>>()
            .join("; ");
        Self { http, base_url, cookie_header }
    }

    /// GET `path` (starting with `/learn/api/`). A 401/403, or a redirect that lands
    /// outside the API (the login page), means the session is no longer valid.
    async fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T, LmsError> {
        let expired = || LmsError::Auth("Your Blackboard session has expired".into());
        let response = self
            .http
            .get(format!("{}{}", self.base_url, path))
            .header(reqwest::header::COOKIE, &self.cookie_header)
            .header(reqwest::header::ACCEPT, "application/json")
            .timeout(REQUEST_TIMEOUT)
            .send()
            .await
            .map_err(|e| connection_error(&e))?;
        let status = response.status();
        if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
            return Err(expired());
        }
        if !response.url().path().starts_with(API_PREFIX) {
            return Err(expired());
        }
        let endpoint = path.split('?').next().unwrap_or(path);
        if !status.is_success() {
            return Err(LmsError::Protocol(format!("{endpoint} returned HTTP {}", status.as_u16())));
        }
        response
            .json()
            .await
            .map_err(|_| LmsError::Protocol(format!("{endpoint} returned an unexpected response")))
    }

    async fn get_all<T: DeserializeOwned>(&self, first: &str) -> Result<Vec<T>, LmsError> {
        let mut items = Vec::new();
        let mut next = Some(first.to_string());
        let mut pages = 0;
        while let Some(path) = next.take() {
            pages += 1;
            if pages > MAX_PAGES {
                return Err(LmsError::Protocol("Blackboard returned too many pages".into()));
            }
            let page: Page<T> = self.get(&path).await?;
            items.extend(page.results);
            next = page.paging.and_then(|p| p.next_page).filter(|p| p.starts_with(API_PREFIX));
        }
        Ok(items)
    }
}

#[async_trait]
impl LmsClient for BlackboardClient {
    async fn verify(&self) -> Result<LmsIdentity, LmsError> {
        let me: Me = self.get("/learn/api/public/v1/users/me").await?;
        let full_name = me
            .name
            .map(|n| format!("{} {}", n.given.unwrap_or_default(), n.family.unwrap_or_default()))
            .map(|n| n.trim().to_string())
            .filter(|n| !n.is_empty());
        let display_name = full_name.or(me.user_name).unwrap_or_else(|| me.id.clone());
        Ok(LmsIdentity { user_id: me.id, display_name })
    }

    async fn list_courses(&self) -> Result<Vec<RemoteCourse>, LmsError> {
        let memberships: Vec<Membership> = self
            .get_all("/learn/api/public/v1/users/me/courses?expand=course&limit=100")
            .await?;
        Ok(memberships
            .into_iter()
            .filter_map(|m| {
                let course = m.course?;
                Some(RemoteCourse {
                    url: Some(format!("{}/ultra/courses/{}/outline", self.base_url, m.course_id)),
                    external_id: m.course_id,
                    short_name: course.code.filter(|c| !c.trim().is_empty()),
                    name: course.name,
                })
            })
            .collect())
    }
}
```

Run: `cargo test -p academic-infra --test lms_blackboard` → PASS.

- [ ] **Step 8: Probe** — tests `crates/infra/tests/lms_probe.rs`:

```rust
use academic_core::domain::LmsPlatform;
use academic_infra::lms::probe::{probe, ProbeError};
use serde_json::json;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

async fn moodle_config(server: &MockServer, data: serde_json::Value) {
    Mock::given(method("POST"))
        .and(path("/lib/ajax/service-nologin.php"))
        .and(query_param("info", "tool_mobile_get_public_config"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([{ "error": false, "data": data }])))
        .mount(server)
        .await;
}

#[tokio::test]
async fn moodle_with_mobile_services_passes() {
    let server = MockServer::start().await;
    moodle_config(&server, json!({ "enablewebservices": 1, "enablemobilewebservice": 1 })).await;
    probe(&reqwest::Client::new(), LmsPlatform::Moodle, &server.uri()).await.unwrap();
}

#[tokio::test]
async fn moodle_without_mobile_services_is_a_mismatch_with_a_clear_message() {
    let server = MockServer::start().await;
    moodle_config(&server, json!({ "enablewebservices": 1, "enablemobilewebservice": 0 })).await;
    match probe(&reqwest::Client::new(), LmsPlatform::Moodle, &server.uri()).await {
        Err(ProbeError::Mismatch(m)) => assert_eq!(m, "Mobile web services are disabled on this Moodle site"),
        other => panic!("{other:?}"),
    }
}

#[tokio::test]
async fn a_non_moodle_site_is_a_mismatch() {
    let server = MockServer::start().await;
    Mock::given(method("POST")).respond_with(ResponseTemplate::new(404)).mount(&server).await;
    match probe(&reqwest::Client::new(), LmsPlatform::Moodle, &server.uri()).await {
        Err(ProbeError::Mismatch(m)) => assert_eq!(m, "This URL is not a Moodle site"),
        other => panic!("{other:?}"),
    }
}

#[tokio::test]
async fn blackboard_version_endpoint_passes_and_others_mismatch() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/learn/api/public/v1/system/version"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "learn": { "major": 4001 } })))
        .mount(&server)
        .await;
    probe(&reqwest::Client::new(), LmsPlatform::Blackboard, &server.uri()).await.unwrap();

    let other = MockServer::start().await;
    match probe(&reqwest::Client::new(), LmsPlatform::Blackboard, &other.uri()).await {
        Err(ProbeError::Mismatch(m)) => assert_eq!(m, "This URL is not a Blackboard Learn site"),
        r => panic!("{r:?}"),
    }
}

#[tokio::test]
async fn unreachable_host_is_a_connection_error() {
    let result = probe(&reqwest::Client::new(), LmsPlatform::Moodle, "https://127.0.0.1:9").await;
    assert!(matches!(result, Err(ProbeError::Connection(_))));
}
```

Implementation `lms/probe.rs`:

```rust
//! Unauthenticated check that a URL hosts the chosen platform (spec §5.3).

use academic_core::domain::LmsPlatform;
use serde_json::{json, Value};

use super::REQUEST_TIMEOUT;
use crate::http_util::describe_transport_error;

#[derive(Debug, thiserror::Error)]
pub enum ProbeError {
    #[error("{0}")]
    Mismatch(String),
    #[error("Could not reach the platform: {0}")]
    Connection(String),
}

pub async fn probe(http: &reqwest::Client, platform: LmsPlatform, base_url: &str) -> Result<(), ProbeError> {
    match platform {
        LmsPlatform::Moodle => probe_moodle(http, base_url).await,
        LmsPlatform::Blackboard => probe_blackboard(http, base_url).await,
    }
}

async fn probe_moodle(http: &reqwest::Client, base_url: &str) -> Result<(), ProbeError> {
    let not_moodle = || ProbeError::Mismatch("This URL is not a Moodle site".into());
    let response = http
        .post(format!("{base_url}/lib/ajax/service-nologin.php?info=tool_mobile_get_public_config"))
        .json(&json!([{ "index": 0, "methodname": "tool_mobile_get_public_config", "args": {} }]))
        .timeout(REQUEST_TIMEOUT)
        .send()
        .await
        .map_err(|e| ProbeError::Connection(describe_transport_error(&e)))?;
    if !response.status().is_success() {
        return Err(not_moodle());
    }
    let body: Value = response.json().await.map_err(|_| not_moodle())?;
    let first = body.get(0).ok_or_else(not_moodle)?;
    if first.get("error").and_then(Value::as_bool) != Some(false) {
        return Err(not_moodle());
    }
    let data = first.get("data").ok_or_else(not_moodle)?;
    if is_enabled(data.get("enablewebservices")) && is_enabled(data.get("enablemobilewebservice")) {
        Ok(())
    } else {
        Err(ProbeError::Mismatch("Mobile web services are disabled on this Moodle site".into()))
    }
}

/// Moodle reports these flags as `1`/`0` (sometimes as booleans).
fn is_enabled(value: Option<&Value>) -> bool {
    matches!(value, Some(v) if v.as_i64() == Some(1) || v.as_bool() == Some(true))
}

async fn probe_blackboard(http: &reqwest::Client, base_url: &str) -> Result<(), ProbeError> {
    let not_blackboard = || ProbeError::Mismatch("This URL is not a Blackboard Learn site".into());
    let response = http
        .get(format!("{base_url}/learn/api/public/v1/system/version"))
        .timeout(REQUEST_TIMEOUT)
        .send()
        .await
        .map_err(|e| ProbeError::Connection(describe_transport_error(&e)))?;
    if !response.status().is_success() {
        return Err(not_blackboard());
    }
    let body: Value = response.json().await.map_err(|_| not_blackboard())?;
    if body.get("learn").is_some_and(Value::is_object) { Ok(()) } else { Err(not_blackboard()) }
}
```

Run: `cargo test -p academic-infra --test lms_probe` → PASS.

- [ ] **Step 9: Factory** — tests `crates/infra/tests/lms_factory.rs`:

```rust
use std::sync::Arc;

use academic_core::domain::{lms_credential_key, LmsPlatform, LmsUniversity, LoginState};
use academic_core::ports::{LmsClientFactory, LmsError, SecretStore};
use academic_infra::lms::factory::DefaultLmsClientFactory;
use academic_infra::secrets::InMemorySecretStore;
use chrono::Utc;

fn uni(platform: LmsPlatform) -> LmsUniversity {
    LmsUniversity {
        id: 5,
        name: "X".into(),
        platform,
        base_url: "https://x.es".into(),
        login_state: LoginState::Ok,
        account_label: None,
        last_sync_at: None,
        last_sync_outcome: None,
        last_sync_error: None,
        created_at: Utc::now(),
    }
}

fn factory(secrets: Arc<InMemorySecretStore>) -> DefaultLmsClientFactory {
    DefaultLmsClientFactory::new(secrets, reqwest::Client::new())
}

#[test]
fn missing_credential_is_an_auth_error() {
    let f = factory(Arc::new(InMemorySecretStore::default()));
    assert!(matches!(f.create(&uni(LmsPlatform::Moodle)), Err(LmsError::Auth(_))));
    assert!(matches!(f.create(&uni(LmsPlatform::Blackboard)), Err(LmsError::Auth(_))));
}

#[test]
fn unreadable_blackboard_session_is_an_auth_error() {
    let secrets = Arc::new(InMemorySecretStore::default());
    secrets.set(&lms_credential_key(LmsPlatform::Blackboard, 5), "[]garbage").unwrap();
    assert!(matches!(factory(secrets).create(&uni(LmsPlatform::Blackboard)), Err(LmsError::Auth(_))));
}

#[test]
fn stored_credentials_build_clients() {
    let secrets = Arc::new(InMemorySecretStore::default());
    secrets.set(&lms_credential_key(LmsPlatform::Moodle, 5), "tok").unwrap();
    secrets
        .set(&lms_credential_key(LmsPlatform::Blackboard, 5), r#"[{"name":"BbRouter","value":"v"}]"#)
        .unwrap();
    let f = factory(secrets);
    assert!(f.create(&uni(LmsPlatform::Moodle)).is_ok());
    assert!(f.create(&uni(LmsPlatform::Blackboard)).is_ok());
    assert!(f.create_with_credential(&uni(LmsPlatform::Moodle), "fresh").is_ok());
}
```

Implementation `lms/factory.rs`:

```rust
//! Builds the LMS client for a university from its stored credential.

use std::sync::Arc;

use academic_core::domain::{lms_credential_key, LmsPlatform, LmsUniversity};
use academic_core::ports::{LmsClient, LmsClientFactory, LmsError, SecretStore};

use super::blackboard::{decode_session, BlackboardClient};
use super::moodle::MoodleClient;

pub struct DefaultLmsClientFactory {
    secrets: Arc<dyn SecretStore>,
    http: reqwest::Client,
}

impl DefaultLmsClientFactory {
    pub fn new(secrets: Arc<dyn SecretStore>, http: reqwest::Client) -> Self {
        Self { secrets, http }
    }
}

impl LmsClientFactory for DefaultLmsClientFactory {
    fn create(&self, university: &LmsUniversity) -> Result<Box<dyn LmsClient>, LmsError> {
        let credential = self
            .secrets
            .get(&lms_credential_key(university.platform, university.id))
            .map_err(|e| LmsError::Protocol(e.to_string()))?
            .ok_or_else(|| LmsError::Auth(format!("Sign in to {} to continue", university.name)))?;
        self.create_with_credential(university, &credential)
    }

    fn create_with_credential(
        &self,
        university: &LmsUniversity,
        credential: &str,
    ) -> Result<Box<dyn LmsClient>, LmsError> {
        let base_url = university.base_url.clone();
        Ok(match university.platform {
            LmsPlatform::Moodle => {
                Box::new(MoodleClient::new(self.http.clone(), base_url, credential.to_string()))
            }
            LmsPlatform::Blackboard => Box::new(BlackboardClient::new(
                self.http.clone(),
                base_url,
                &decode_session(credential)?,
            )),
        })
    }
}
```

- [ ] **Step 10: Gates and commit**

```bash
git add Cargo.toml Cargo.lock crates/infra
git commit -m "feat(infra): add LMS probe, Moodle and Blackboard clients"
```

---

### Task 4: Login window capture and `TauriLmsAuthenticator`

**Files:**
- Create: `src-tauri/src/lms_auth/mod.rs`, `src-tauri/src/lms_auth/capture.rs` (pure helpers + tests), `src-tauri/src/lms_auth/window.rs` (Tauri window driver)
- Modify: `src-tauri/src/lib.rs` (`mod lms_auth;`)
- Modify: `src-tauri/Cargo.toml` (add `academic-infra` is already there; add `rand.workspace = true`, `async-trait.workspace = true` to `[dependencies]` — move `async-trait` from dev-dependencies)

**Interfaces:**
- Consumes: Task 1 (`LmsUniversity`, `LmsPlatform`, `LmsError`, `LmsAuthenticator`, `LmsClientFactory`, `SecretStore`, `lms::adopt_credential`, `SessionError`), Task 3 (`decode_launch_token`, `SessionCookie`, `encode_session`, `MAX_SECRET_BYTES`).
- Produces:
  - `lms_auth::LoginMode::{Interactive, Silent}`
  - `lms_auth::capture_credential(app: &tauri::AppHandle, university: &LmsUniversity, mode: LoginMode) -> Result<String, LmsError>` — returns the raw credential (Moodle token, or Blackboard session JSON), not yet verified or stored.
  - `lms_auth::TauriLmsAuthenticator::new(app: AppHandle, factory: Arc<dyn LmsClientFactory>, secrets: Arc<dyn SecretStore>)` implementing `LmsAuthenticator` (capture silently → `adopt_credential`).

- [ ] **Step 1: Pure helpers with tests** — `capture.rs`:

```rust
//! Pure helpers for the LMS login window: start URLs, redirect/landing detection,
//! and turning captured cookies into the stored session (spec §6).

use academic_infra::lms::blackboard::{encode_session, SessionCookie};
use academic_infra::secrets::MAX_SECRET_BYTES;
use rand::distr::{Alphanumeric, SampleString};

const MOODLE_TOKEN_PREFIX: &str = "moodlemobile://token=";
const PASSPORT_LENGTH: usize = 32;

pub fn random_passport() -> String {
    Alphanumeric.sample_string(&mut rand::rng(), PASSPORT_LENGTH)
}

pub fn moodle_launch_url(base_url: &str, passport: &str) -> String {
    format!(
        "{base_url}/admin/tool/mobile/launch.php?service=moodle_mobile_app&passport={passport}&urlscheme=moodlemobile"
    )
}

/// The payload after `moodlemobile://token=`, if `url` is the launch redirect.
pub fn moodle_token_payload(url: &str) -> Option<&str> {
    url.strip_prefix(MOODLE_TOKEN_PREFIX)
}

pub fn blackboard_start_url(base_url: &str) -> String {
    format!("{base_url}/ultra")
}

/// True once the window shows an Ultra page (e.g. `/ultra/stream`), which only
/// happens after a successful sign-in. `/ultra` itself redirects to the login page.
pub fn is_blackboard_signed_in(url: &str, base_url: &str) -> bool {
    url.strip_prefix(base_url)
        .is_some_and(|path| path.starts_with("/ultra/") && path.len() > "/ultra/".len())
}

/// A cookie as read from the webview: name, value and optional domain attribute.
pub struct RawCookie {
    pub name: String,
    pub value: String,
    pub domain: Option<String>,
}

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum CookieCaptureError {
    #[error("No Blackboard session was found after sign-in")]
    Empty,
    /// Holds the cookie names (never values) for the log.
    #[error("The Blackboard session is too large to store securely")]
    TooLarge(Vec<String>),
}

/// Keeps the cookies that belong to `host` and encodes them as the stored session.
pub fn encode_session_cookies(cookies: Vec<RawCookie>, host: &str) -> Result<String, CookieCaptureError> {
    let kept: Vec<SessionCookie> = cookies
        .into_iter()
        .filter(|c| domain_matches(c.domain.as_deref(), host))
        .map(|c| SessionCookie { name: c.name, value: c.value })
        .collect();
    if kept.is_empty() {
        return Err(CookieCaptureError::Empty);
    }
    let encoded = encode_session(&kept);
    if encoded.len() > MAX_SECRET_BYTES {
        return Err(CookieCaptureError::TooLarge(kept.into_iter().map(|c| c.name).collect()));
    }
    Ok(encoded)
}

fn domain_matches(domain: Option<&str>, host: &str) -> bool {
    match domain.map(|d| d.trim_start_matches('.').to_ascii_lowercase()) {
        None => true,
        Some(d) => host == d || host.ends_with(&format!(".{d}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raw(name: &str, value: &str, domain: Option<&str>) -> RawCookie {
        RawCookie { name: name.into(), value: value.into(), domain: domain.map(str::to_string) }
    }

    #[test]
    fn moodle_urls_and_payload_detection() {
        assert_eq!(
            moodle_launch_url("https://aulavirtual.uv.es", "abc"),
            "https://aulavirtual.uv.es/admin/tool/mobile/launch.php?service=moodle_mobile_app&passport=abc&urlscheme=moodlemobile"
        );
        assert_eq!(moodle_token_payload("moodlemobile://token=QUJD"), Some("QUJD"));
        assert_eq!(moodle_token_payload("https://aulavirtual.uv.es/my/"), None);
        let passport = random_passport();
        assert_eq!(passport.len(), 32);
        assert!(passport.chars().all(|c| c.is_ascii_alphanumeric()));
    }

    #[test]
    fn blackboard_landing_detection() {
        let base = "https://uah.blackboard.com";
        assert!(is_blackboard_signed_in("https://uah.blackboard.com/ultra/stream", base));
        assert!(!is_blackboard_signed_in("https://uah.blackboard.com/ultra", base));
        assert!(!is_blackboard_signed_in("https://uah.blackboard.com/ultra/", base));
        assert!(!is_blackboard_signed_in("https://uah.blackboard.com/webapps/login/?action=relogin", base));
        assert!(!is_blackboard_signed_in("https://login.microsoftonline.com/ultra/x", base));
    }

    #[test]
    fn only_cookies_for_the_platform_host_are_kept() {
        let json = encode_session_cookies(
            vec![
                raw("BbRouter", "v1", Some("uah.blackboard.com")),
                raw("JSESSIONID", "v2", None),
                raw("AWSALB", "v3", Some(".blackboard.com")),
                raw("ESTSAUTH", "ms", Some(".login.microsoftonline.com")),
            ],
            "uah.blackboard.com",
        )
        .unwrap();
        assert!(json.contains("BbRouter") && json.contains("JSESSIONID") && json.contains("AWSALB"));
        assert!(!json.contains("ESTSAUTH"));
    }

    #[test]
    fn empty_or_oversized_sessions_are_rejected_without_values() {
        assert_eq!(
            encode_session_cookies(vec![raw("x", "y", Some("other.com"))], "uah.blackboard.com"),
            Err(CookieCaptureError::Empty)
        );
        let big = "v".repeat(MAX_SECRET_BYTES);
        assert_eq!(
            encode_session_cookies(vec![raw("BbRouter", &big, None)], "uah.blackboard.com"),
            Err(CookieCaptureError::TooLarge(vec!["BbRouter".into()]))
        );
    }
}
```

`rand 0.10` API: if `rand::distr::Alphanumeric`/`SampleString`/`rand::rng()` differ in the installed version, check how `crates/infra/src/graph/oauth.rs` generates its PKCE verifier and use the same API.

Run: `cargo test -p academic-assistant lms_auth::capture` → PASS (after Step 1 compiles; write tests first, run to see them fail, then the functions).

- [ ] **Step 2: Window driver** — `window.rs`. This code needs a running app; it is verified by the M1 checklist (Task 7), not by unit tests. Implement exactly this behavior:

```rust
//! Opens the platform's sign-in page in a dedicated window with a persistent,
//! separate browser profile and captures the credential (spec §6.1–§6.3, §6.5).
//! Never logs URLs seen in the window.

use std::sync::Arc;
use std::time::Duration;

use academic_core::domain::{LmsPlatform, LmsUniversity};
use academic_core::lms::{adopt_credential, SessionError};
use academic_core::ports::{LmsAuthenticator, LmsClientFactory, LmsError, SecretStore};
use academic_infra::lms::moodle::decode_launch_token;
use async_trait::async_trait;
use tauri::webview::PageLoadEvent;
use tauri::{AppHandle, Manager, Url, WebviewUrl, WebviewWindow, WebviewWindowBuilder, WindowEvent};
use tokio::sync::mpsc;

use super::capture::{
    blackboard_start_url, encode_session_cookies, is_blackboard_signed_in, moodle_launch_url,
    moodle_token_payload, random_passport, RawCookie,
};

const AUTH_PROFILE_DIR: &str = "lms-auth";
const INTERACTIVE_TIMEOUT: Duration = Duration::from_secs(10 * 60);
const SILENT_TIMEOUT: Duration = Duration::from_secs(20);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoginMode {
    Interactive,
    Silent,
}

enum Signal {
    MoodlePayload(String),
    BlackboardSignedIn,
    Closed,
}

pub async fn capture_credential(
    app: &AppHandle,
    university: &LmsUniversity,
    mode: LoginMode,
) -> Result<String, LmsError> {
    let label = format!("lms-login-{}", university.id);
    if let Some(existing) = app.get_webview_window(&label) {
        let _ = existing.set_focus();
        return Err(LmsError::Auth("A sign-in window for this university is already open".into()));
    }
    let profile = app
        .path()
        .app_data_dir()
        .map_err(|e| LmsError::Protocol(format!("App data directory unavailable: {e}")))?
        .join(AUTH_PROFILE_DIR);
    let passport = random_passport();
    let base_url = university.base_url.clone();
    let start = match university.platform {
        LmsPlatform::Moodle => moodle_launch_url(&base_url, &passport),
        LmsPlatform::Blackboard => blackboard_start_url(&base_url),
    };
    let start: Url = start.parse().map_err(|_| LmsError::Protocol("Invalid platform URL".into()))?;

    let (tx, mut rx) = mpsc::unbounded_channel::<Signal>();
    let mut builder = WebviewWindowBuilder::new(app, &label, WebviewUrl::External(start))
        .title(format!("Sign in to {}", university.name))
        .inner_size(520.0, 720.0)
        .data_directory(profile)
        .visible(mode == LoginMode::Interactive);
    builder = match university.platform {
        LmsPlatform::Moodle => {
            let tx = tx.clone();
            builder.on_navigation(move |url| match moodle_token_payload(url.as_str()) {
                Some(payload) => {
                    let _ = tx.send(Signal::MoodlePayload(payload.to_string()));
                    false // never navigate to the custom scheme
                }
                None => true,
            })
        }
        LmsPlatform::Blackboard => {
            let tx = tx.clone();
            let base_url = base_url.clone();
            builder.on_page_load(move |_window, payload| {
                if payload.event() == PageLoadEvent::Finished
                    && is_blackboard_signed_in(payload.url().as_str(), &base_url)
                {
                    let _ = tx.send(Signal::BlackboardSignedIn);
                }
            })
        }
    };
    let window = builder
        .build()
        .map_err(|e| LmsError::Protocol(format!("Could not open the sign-in window: {e}")))?;
    {
        let tx = tx.clone();
        window.on_window_event(move |event| {
            if matches!(event, WindowEvent::Destroyed) {
                let _ = tx.send(Signal::Closed);
            }
        });
    }
    tracing::info!(university_id = university.id, ?mode, "lms sign-in window opened");

    let timeout = match mode {
        LoginMode::Interactive => INTERACTIVE_TIMEOUT,
        LoginMode::Silent => SILENT_TIMEOUT,
    };
    let signal = tokio::time::timeout(timeout, rx.recv()).await;
    let result = match signal {
        Ok(Some(Signal::MoodlePayload(payload))) => decode_launch_token(&payload, &base_url, &passport),
        Ok(Some(Signal::BlackboardSignedIn)) => read_session(&window, &base_url).await,
        Ok(Some(Signal::Closed)) | Ok(None) => Err(LmsError::Auth("Sign-in cancelled".into())),
        Err(_) => Err(LmsError::Auth(match mode {
            LoginMode::Interactive => "Sign-in timed out".into(),
            LoginMode::Silent => "Sign-in required".into(),
        })),
    };
    let _ = window.destroy();
    let stage = if result.is_ok() { "captured" } else { "not captured" };
    tracing::info!(university_id = university.id, stage, "lms sign-in window closed");
    result
}

/// Reads the window's cookies off the main thread (WebView2 deadlocks when cookies
/// are read from the event loop thread) and encodes the platform host's cookies.
async fn read_session(window: &WebviewWindow, base_url: &str) -> Result<String, LmsError> {
    let url: Url = base_url.parse().map_err(|_| LmsError::Protocol("Invalid platform URL".into()))?;
    let host = url.host_str().unwrap_or_default().to_ascii_lowercase();
    let window = window.clone();
    let cookies = tauri::async_runtime::spawn_blocking(move || window.cookies_for_url(url))
        .await
        .map_err(|_| LmsError::Protocol("Could not read the Blackboard session".into()))?
        .map_err(|_| LmsError::Protocol("Could not read the Blackboard session".into()))?;
    let raw = cookies
        .into_iter()
        .map(|c| RawCookie {
            name: c.name().to_string(),
            value: c.value().to_string(),
            domain: c.domain().map(str::to_string),
        })
        .collect();
    encode_session_cookies(raw, &host).map_err(|error| {
        if let super::capture::CookieCaptureError::TooLarge(names) = &error {
            tracing::warn!(cookie_names = ?names, "blackboard session exceeds the secret size limit");
        }
        LmsError::Auth(error.to_string())
    })
}

/// Silent renewal for `ensure_session` (spec §6.5).
pub struct TauriLmsAuthenticator {
    app: AppHandle,
    factory: Arc<dyn LmsClientFactory>,
    secrets: Arc<dyn SecretStore>,
}

impl TauriLmsAuthenticator {
    pub fn new(app: AppHandle, factory: Arc<dyn LmsClientFactory>, secrets: Arc<dyn SecretStore>) -> Self {
        Self { app, factory, secrets }
    }
}

#[async_trait]
impl LmsAuthenticator for TauriLmsAuthenticator {
    async fn renew_silently(&self, university: &LmsUniversity) -> Result<(), LmsError> {
        let credential = capture_credential(&self.app, university, LoginMode::Silent).await?;
        adopt_credential(university, &credential, self.factory.as_ref(), self.secrets.as_ref())
            .await
            .map(|_| ())
            .map_err(|error| match error {
                SessionError::Lms(e) => e,
                other => LmsError::Protocol(other.to_string()),
            })
    }
}
```

If an API name differs in Tauri 2.12 (e.g. `PageLoadEvent` path, `on_window_event` on `WebviewWindow`, cookie accessor names), use the equivalent from the installed crate (`~/.cargo/registry/src/*/tauri-2.12.0/src/webview/`) and report the difference. Do not add Tauri plugins or capabilities. `mod.rs`:

```rust
//! Embedded SSO sign-in for LMS platforms (spec §6).

mod capture;
mod window;

pub use window::{capture_credential, LoginMode, TauriLmsAuthenticator};
```

- [ ] **Step 3: Gates and commit**

`cargo clippy` may flag `capture_credential`/`TauriLmsAuthenticator` as unused until Task 5 wires them: add `#[allow(dead_code)]` on the `mod lms_auth;` line in `lib.rs` with a comment `// wired in by the university commands (M1 Task 5)`; Task 5 removes it.

```bash
git add src-tauri
git commit -m "feat(app): add LMS sign-in window capture and silent renewal"
```

---

### Task 5: University commands, DTOs and wiring

**Files:**
- Create: `src-tauri/src/commands/universities.rs`, `src-tauri/src/dto/lms.rs`
- Modify: `src-tauri/src/commands/mod.rs`, `src-tauri/src/dto/mod.rs`, `src-tauri/src/lib.rs` (register commands; remove Task 4's `#[allow(dead_code)]`), `src-tauri/src/state.rs`, `src-tauri/src/setup.rs`, `src-tauri/src/error.rs`, `src-tauri/src/validation.rs`, `src-tauri/src/test_support.rs`

**Interfaces:**
- Consumes: Tasks 1–4.
- Produces — commands (all `async`, returning `Result<_, CommandError>`) and DTOs; Task 6 relies on these exact shapes:

```ts
type LmsPlatform = 'moodle' | 'blackboard';
type LoginState = 'never' | 'ok' | 'expired' | 'error';
type SyncOutcome = 'success' | 'failed' | 'login_required';
interface LmsUniversityDto {
  id: number; name: string; platform: LmsPlatform; baseUrl: string;
  loginState: LoginState; accountLabel: string | null;
  lastSync: { at: string | null; outcome: SyncOutcome | null; error: string | null };
  courses: { tracked: number; total: number; unreviewed: number };
}
interface LmsUniversityInput { name: string; platform: LmsPlatform; baseUrl: string }
interface ProbeDto { baseUrl: string }
```

| Command | Args (JS keys) | Returns |
|---|---|---|
| `lms_probe` | `{ url, platform }` | `ProbeDto` |
| `get_universities` | `{}` | `LmsUniversityDto[]` |
| `add_university` | `{ input: LmsUniversityInput }` | `LmsUniversityDto` |
| `rename_university` | `{ id, name }` | `LmsUniversityDto` |
| `remove_university` | `{ id }` | `null` |
| `lms_sign_in` | `{ id }` | `LmsUniversityDto` |
| `lms_sign_out` | `{ id }` | `LmsUniversityDto` |
| `lms_check_session` | `{ id }` | `LmsUniversityDto` |

- [ ] **Step 1: State and wiring.** Add to `AppState` (and `Collaborators`): `universities: Arc<dyn UniversityRepository>`, `courses: Arc<dyn CourseRepository>` (both from `db`), `lms_factory: Arc<dyn LmsClientFactory>`, `lms_authenticator: Arc<dyn LmsAuthenticator>`. In `setup.rs::build_state`: `let lms_factory = Arc::new(DefaultLmsClientFactory::new(secrets.clone(), http.clone()));` and `lms_authenticator: Arc::new(TauriLmsAuthenticator::new(app.handle().clone(), lms_factory.clone(), secrets.clone()))`. In `test_support.rs`, add configurable fakes:

```rust
/// Scripted LMS behavior for command tests.
#[derive(Clone, Copy)]
pub struct FakeLmsBehavior {
    pub verify: fn() -> Result<LmsIdentity, LmsError>,
    pub renew: fn() -> Result<(), LmsError>,
}

impl Default for FakeLmsBehavior {
    fn default() -> Self {
        Self {
            verify: || Ok(LmsIdentity { user_id: "1".into(), display_name: "Ana Pérez".into() }),
            renew: || Err(LmsError::Auth("Sign-in required".into())),
        }
    }
}
```

with `FakeLmsFactory(FakeLmsBehavior)` (both `create` and `create_with_credential` return a client whose `verify` calls `behavior.verify` and whose `list_courses` returns `Ok(vec![])`) and `FakeLmsAuthenticator(FakeLmsBehavior)`. `harness()` uses `FakeLmsBehavior::default()`; add `harness_with_lms(behavior: FakeLmsBehavior) -> Harness`. Real `create` must still fail with `Auth` when no secret is stored: the fake factory checks `secrets.get(lms_credential_key(..))` and returns `LmsError::Auth("Sign in")` if absent, so tests exercise the renewal path realistically (pass the `InMemorySecretStore` into the fake).

- [ ] **Step 2: Error mapping** in `error.rs` (with tests next to the existing ones):

```rust
impl From<LmsError> for CommandError {
    fn from(error: LmsError) -> Self {
        let kind = match error {
            LmsError::Auth(_) => CommandErrorKind::Auth,
            LmsError::Connection(_) => CommandErrorKind::Connection,
            LmsError::Protocol(_) => CommandErrorKind::Internal,
        };
        Self::new(kind, error.to_string())
    }
}

impl From<SessionError> for CommandError {
    fn from(error: SessionError) -> Self {
        match error {
            SessionError::LoginRequired(_) => Self::new(CommandErrorKind::Auth, error.to_string()),
            SessionError::Lms(e) => e.into(),
            SessionError::Storage(detail) => Self::internal(STORAGE_MESSAGE, detail),
            SessionError::Secret(detail) => Self::internal(SECRET_STORAGE_MESSAGE, detail),
        }
    }
}
```

Tests: `LmsError::Auth("Sign-in cancelled")` → kind `auth`, message `"Sign-in cancelled"`; `SessionError::LoginRequired("UAH")` → kind `auth`, message `"UAH needs you to sign in again"`; `SessionError::Storage("x")` → internal with `STORAGE_MESSAGE`.

- [ ] **Step 3: DTOs** — `dto/lms.rs`:

```rust
//! LMS DTOs (spec §9.1, §18).

use academic_core::domain::{LmsCourse, LmsPlatform, LmsUniversity, LoginState, SyncOutcome};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LastSyncDto {
    pub at: Option<String>,
    pub outcome: Option<SyncOutcome>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CourseCountsDto {
    pub tracked: usize,
    pub total: usize,
    pub unreviewed: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LmsUniversityDto {
    pub id: i64,
    pub name: String,
    pub platform: LmsPlatform,
    pub base_url: String,
    pub login_state: LoginState,
    pub account_label: Option<String>,
    pub last_sync: LastSyncDto,
    pub courses: CourseCountsDto,
}

impl LmsUniversityDto {
    pub fn new(university: &LmsUniversity, courses: &[LmsCourse]) -> Self {
        Self {
            id: university.id,
            name: university.name.clone(),
            platform: university.platform,
            base_url: university.base_url.clone(),
            login_state: university.login_state,
            account_label: university.account_label.clone(),
            last_sync: LastSyncDto {
                at: university.last_sync_at.map(|at| at.to_rfc3339()),
                outcome: university.last_sync_outcome,
                error: university.last_sync_error.clone(),
            },
            courses: CourseCountsDto {
                tracked: courses.iter().filter(|c| c.tracked).count(),
                total: courses.len(),
                unreviewed: courses.iter().filter(|c| !c.reviewed).count(),
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LmsUniversityInput {
    pub name: String,
    pub platform: LmsPlatform,
    pub base_url: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProbeDto {
    pub base_url: String,
}
```

Use the same RFC 3339 formatting helper the other DTOs use (check `dto/runs.rs`) instead of `to_rfc3339()` if one exists. Add a serialization test asserting the exact JSON of a sample `LmsUniversityDto` (keys `baseUrl`, `loginState`, `accountLabel`, `lastSync.{at,outcome,error}`, `courses.{tracked,total,unreviewed}`, platform `"moodle"`).

- [ ] **Step 4: Write failing command tests** in `commands/universities.rs` (test module), using the harness:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::CommandErrorKind;
    use crate::test_support::{harness, harness_with_lms, FakeLmsBehavior};
    use academic_core::domain::{lms_credential_key, LmsPlatform};
    use academic_core::ports::{LmsError, SecretStore};

    fn add(h: &crate::test_support::Harness, name: &str, url: &str) -> LmsUniversityDto {
        insert_validated(&h.state, name, LmsPlatform::Moodle, url).unwrap()
    }

    #[test]
    fn names_are_required_and_urls_normalized() {
        let h = harness();
        assert_eq!(insert_validated(&h.state, "  ", LmsPlatform::Moodle, "aulavirtual.uv.es").unwrap_err().message, "Enter a name");
        let error = insert_validated(&h.state, "UV", LmsPlatform::Moodle, "http://aulavirtual.uv.es").unwrap_err();
        assert_eq!(error.kind, CommandErrorKind::Validation);
        let u = add(&h, " UV ", "https://aulavirtual.uv.es/my/");
        assert_eq!((u.name.as_str(), u.base_url.as_str()), ("UV", "https://aulavirtual.uv.es"));
        assert_eq!(u.login_state, LoginState::Never);
    }

    #[test]
    fn the_same_platform_cannot_be_added_twice() {
        let h = harness();
        add(&h, "UV", "https://aulavirtual.uv.es");
        let error = insert_validated(&h.state, "Valencia", LmsPlatform::Moodle, "aulavirtual.uv.es/").unwrap_err();
        assert_eq!(error.kind, CommandErrorKind::Conflict);
        assert_eq!(error.message, "This platform is already added");
        assert_eq!(universities(&h.state).unwrap().len(), 1);
    }

    #[tokio::test]
    async fn successful_sign_in_stores_the_credential_and_marks_ok() {
        let h = harness();
        let u = add(&h, "UV", "https://aulavirtual.uv.es");
        let dto = complete_sign_in(&h.state, u.id, Ok("tok".into())).await.unwrap();
        assert_eq!(dto.login_state, LoginState::Ok);
        assert_eq!(dto.account_label.as_deref(), Some("Ana Pérez"));
        assert_eq!(h.secrets.get(&lms_credential_key(LmsPlatform::Moodle, u.id)).unwrap().as_deref(), Some("tok"));
    }

    #[tokio::test]
    async fn cancelled_sign_in_changes_nothing() {
        let h = harness();
        let u = add(&h, "UV", "https://aulavirtual.uv.es");
        let error = complete_sign_in(&h.state, u.id, Err(LmsError::Auth("Sign-in cancelled".into()))).await.unwrap_err();
        assert_eq!((error.kind, error.message.as_str()), (CommandErrorKind::Auth, "Sign-in cancelled"));
        assert_eq!(universities(&h.state).unwrap()[0].login_state, LoginState::Never);
    }

    #[tokio::test]
    async fn rejected_credential_marks_error_and_is_not_stored() {
        let h = harness_with_lms(FakeLmsBehavior {
            verify: || Err(LmsError::Auth("rejected".into())),
            ..FakeLmsBehavior::default()
        });
        let u = add(&h, "UV", "https://aulavirtual.uv.es");
        assert!(complete_sign_in(&h.state, u.id, Ok("tok".into())).await.is_err());
        assert_eq!(universities(&h.state).unwrap()[0].login_state, LoginState::Error);
        assert_eq!(h.secrets.get(&lms_credential_key(LmsPlatform::Moodle, u.id)).unwrap(), None);
    }

    #[tokio::test]
    async fn check_session_without_secret_tries_renewal_then_reports_login_required() {
        let h = harness();
        let u = add(&h, "UV", "https://aulavirtual.uv.es");
        h.state.universities.set_login(u.id, LoginState::Ok, Some("Ana Pérez")).unwrap();
        let error = check_session(&h.state, u.id).await.unwrap_err();
        assert_eq!(error.kind, CommandErrorKind::Auth);
        assert_eq!(universities(&h.state).unwrap()[0].login_state, LoginState::Expired);
    }

    #[test]
    fn sign_out_and_remove_delete_the_secret() {
        let h = harness();
        let u = add(&h, "UV", "https://aulavirtual.uv.es");
        let key = lms_credential_key(LmsPlatform::Moodle, u.id);
        h.secrets.set(&key, "tok").unwrap();
        let dto = sign_out(&h.state, u.id).unwrap();
        assert_eq!(dto.login_state, LoginState::Never);
        assert_eq!(h.secrets.get(&key).unwrap(), None);

        h.secrets.set(&key, "tok").unwrap();
        remove(&h.state, u.id).unwrap();
        assert_eq!(h.secrets.get(&key).unwrap(), None);
        assert!(universities(&h.state).unwrap().is_empty());
    }

    #[test]
    fn missing_university_is_not_found() {
        let h = harness();
        assert_eq!(sign_out(&h.state, 99).unwrap_err().kind, CommandErrorKind::NotFound);
        assert_eq!(rename(&h.state, 99, "x").unwrap_err().kind, CommandErrorKind::NotFound);
    }
}
```

- [ ] **Step 5: Implement the commands.** Thin `#[tauri::command]` wrappers around testable functions, following `commands/accounts.rs` (`logged(...)`):

```rust
//! Commands: LMS universities, sign-in and session check (spec §9.2, §18.4).

use academic_core::domain::{lms_credential_key, LmsPlatform, LmsUniversity, LoginState, UniversityId};
use academic_core::lms::{adopt_credential, ensure_session, normalize_base_url};
use academic_core::ports::LmsError;
use academic_infra::lms::probe::{probe, ProbeError};
use tauri::{AppHandle, State};

use super::logged;
use crate::dto::{LmsUniversityDto, LmsUniversityInput, ProbeDto};
use crate::error::CommandError;
use crate::lms_auth::{capture_credential, LoginMode};
use crate::state::AppState;
use crate::validation;

const DUPLICATE_PLATFORM: &str = "This platform is already added";
const NOT_FOUND: &str = "University not found";

#[tauri::command]
pub async fn lms_probe(state: State<'_, AppState>, url: String, platform: LmsPlatform) -> Result<ProbeDto, CommandError> {
    logged("lms_probe", probe_url(&state, &url, platform).await)
}

#[tauri::command]
pub async fn get_universities(state: State<'_, AppState>) -> Result<Vec<LmsUniversityDto>, CommandError> {
    logged("get_universities", universities(&state))
}

#[tauri::command]
pub async fn add_university(state: State<'_, AppState>, input: LmsUniversityInput) -> Result<LmsUniversityDto, CommandError> {
    logged("add_university", async {
        let base = probe_url(&state, &input.base_url, input.platform).await?;
        insert_validated(&state, &input.name, input.platform, &base.base_url)
    }.await)
}

#[tauri::command]
pub async fn rename_university(state: State<'_, AppState>, id: UniversityId, name: String) -> Result<LmsUniversityDto, CommandError> {
    logged("rename_university", rename(&state, id, &name))
}

#[tauri::command]
pub async fn remove_university(state: State<'_, AppState>, id: UniversityId) -> Result<(), CommandError> {
    logged("remove_university", remove(&state, id))
}

#[tauri::command]
pub async fn lms_sign_in(app: AppHandle, state: State<'_, AppState>, id: UniversityId) -> Result<LmsUniversityDto, CommandError> {
    logged("lms_sign_in", async {
        let university = find(&state, id)?;
        let captured = capture_credential(&app, &university, LoginMode::Interactive).await;
        complete_sign_in(&state, id, captured).await
    }.await)
}

#[tauri::command]
pub async fn lms_sign_out(state: State<'_, AppState>, id: UniversityId) -> Result<LmsUniversityDto, CommandError> {
    logged("lms_sign_out", sign_out(&state, id))
}

#[tauri::command]
pub async fn lms_check_session(state: State<'_, AppState>, id: UniversityId) -> Result<LmsUniversityDto, CommandError> {
    logged("lms_check_session", check_session(&state, id).await)
}

fn find(state: &AppState, id: UniversityId) -> Result<LmsUniversity, CommandError> {
    state.universities.get_university(id)?.ok_or_else(|| CommandError::not_found(NOT_FOUND))
}

fn dto(state: &AppState, university: &LmsUniversity) -> Result<LmsUniversityDto, CommandError> {
    Ok(LmsUniversityDto::new(university, &state.courses.list_courses(university.id)?))
}

fn universities(state: &AppState) -> Result<Vec<LmsUniversityDto>, CommandError> {
    state.universities.list_universities()?.iter().map(|u| dto(state, u)).collect()
}

async fn probe_url(state: &AppState, url: &str, platform: LmsPlatform) -> Result<ProbeDto, CommandError> {
    let base_url = normalize_base_url(url).map_err(|e| CommandError::validation(e.to_string()))?;
    probe(&state.http, platform, &base_url).await.map_err(|error| match error {
        ProbeError::Mismatch(message) => CommandError::validation(message),
        ProbeError::Connection(_) => CommandError::from(LmsError::Connection(error.to_string())),
    })?;
    Ok(ProbeDto { base_url })
}

/// Validates and inserts without probing (the command probes first).
fn insert_validated(state: &AppState, name: &str, platform: LmsPlatform, url: &str) -> Result<LmsUniversityDto, CommandError> {
    let name = validation::name(name)?;
    let base_url = normalize_base_url(url).map_err(|e| CommandError::validation(e.to_string()))?;
    if state.universities.list_universities()?.iter().any(|u| u.base_url == base_url) {
        return Err(CommandError::conflict(DUPLICATE_PLATFORM));
    }
    let university = state.universities.insert_university(&name, platform, &base_url, state.clock.now())?;
    dto(state, &university)
}

fn rename(state: &AppState, id: UniversityId, name: &str) -> Result<LmsUniversityDto, CommandError> {
    let name = validation::name(name)?;
    let university = state.universities.rename_university(id, &name)?;
    dto(state, &university)
}

/// Deletes the secret first so a failure never leaves a credential without its university.
fn remove(state: &AppState, id: UniversityId) -> Result<(), CommandError> {
    let university = find(state, id)?;
    state.secrets.delete(&lms_credential_key(university.platform, id))?;
    Ok(state.universities.delete_university(id)?)
}

async fn complete_sign_in(
    state: &AppState,
    id: UniversityId,
    captured: Result<String, LmsError>,
) -> Result<LmsUniversityDto, CommandError> {
    let university = find(state, id)?;
    let credential = captured?;
    match adopt_credential(&university, &credential, state.lms_factory.as_ref(), state.secrets.as_ref()).await {
        Ok(identity) => {
            let updated = state.universities.set_login(id, LoginState::Ok, Some(&identity.display_name))?;
            dto(state, &updated)
        }
        Err(error) => {
            state.universities.set_login(id, LoginState::Error, None)?;
            Err(error.into())
        }
    }
}

fn sign_out(state: &AppState, id: UniversityId) -> Result<LmsUniversityDto, CommandError> {
    let university = find(state, id)?;
    state.secrets.delete(&lms_credential_key(university.platform, id))?;
    let updated = state.universities.set_login(id, LoginState::Never, None)?;
    dto(state, &updated)
}

async fn check_session(state: &AppState, id: UniversityId) -> Result<LmsUniversityDto, CommandError> {
    let university = find(state, id)?;
    let updated = ensure_session(
        &university,
        state.lms_factory.as_ref(),
        state.lms_authenticator.as_ref(),
        state.universities.as_ref(),
    )
    .await?;
    dto(state, &updated)
}
```

`CommandError::conflict(message)` does not exist yet: add `pub fn conflict(message: impl Into<String>) -> Self { Self::new(CommandErrorKind::Conflict, message) }` next to `validation`/`not_found`. A `RepoError::Conflict` from `insert_university` (duplicate name) keeps the generic message "An item with that name already exists". Register the eight commands in `lib.rs`.

- [ ] **Step 6: Run tests and gates; commit**

Run: `cargo test -p academic-assistant` → PASS, then all Rust gates.

```bash
git add src-tauri
git commit -m "feat(app): add university, sign-in and session commands"
```

---

### Task 6: Universities page (frontend) and catalog page removal

**Files:**
- Modify: `src/lib/api/types.ts` (LMS types from Task 5's Interfaces), `src/lib/api/client.ts` (new methods; remove `addUniversity`, `updateUniversity`, `removeUniversity`, `addCourse`, `updateCourse`, `removeCourse`; keep `getCatalog`), `src/lib/api/client.test.ts` (drop tests of removed methods; add tests for new ones), `src/lib/api/queryKeys.ts` (`universities: ['universities'] as const`)
- Create: `src/features/universities/UniversitiesPage.tsx`, `UniversityCard.tsx`, `AddUniversityModal.tsx`, `RenameUniversityModal.tsx`, `labels.ts`, and tests `UniversitiesPage.test.tsx`, `AddUniversityModal.test.tsx`
- Delete: `src/features/catalog/` (whole folder)
- Modify: `src/App.tsx` (route `/universities`; remove `/catalog`), `src/components/Layout.tsx` (nav item `{ to: '/universities', label: 'Universities', end: false, Icon: UniversityIcon }` replacing the catalog item), `src/components/Layout.test.tsx` (labels)

**Interfaces:**
- Consumes: Task 5 command/DTO contract (above). Can be implemented before Task 5 lands because tests mock `api`.
- Produces: `api.lmsProbe(url, platform) → Promise<ProbeDto>`, `api.getUniversities()`, `api.addUniversity(input)`, `api.renameUniversity(id, name)`, `api.removeUniversity(id)`, `api.lmsSignIn(id)`, `api.lmsSignOut(id)`, `api.lmsCheckSession(id)`.

- [ ] **Step 1: Types and client** — append to `types.ts` the TS block from Task 5 (exported). In `client.ts`:

```ts
  getUniversities: () => invoke<LmsUniversityDto[]>('get_universities', {}),

  lmsProbe: (url: string, platform: LmsPlatform) =>
    invoke<ProbeDto>('lms_probe', { url, platform }),

  addUniversity: (input: LmsUniversityInput) =>
    invoke<LmsUniversityDto>('add_university', { input }),

  renameUniversity: (id: number, name: string) =>
    invoke<LmsUniversityDto>('rename_university', { id, name }),

  removeUniversity: (id: number) => invoke<void>('remove_university', { id }),

  lmsSignIn: (id: number) => invoke<LmsUniversityDto>('lms_sign_in', { id }),

  lmsSignOut: (id: number) => invoke<LmsUniversityDto>('lms_sign_out', { id }),

  lmsCheckSession: (id: number) => invoke<LmsUniversityDto>('lms_check_session', { id }),
```

Add one `client.test.ts` case per new method asserting the command name and argument object (follow the existing tests' `invoke` mock), e.g. `addUniversity({ name: 'UV', platform: 'moodle', baseUrl: 'aulavirtual.uv.es' })` → `invoke('add_university', { input: {...} })`.

- [ ] **Step 2: Labels** — `labels.ts`:

```ts
import type { LmsPlatform, LoginState } from '../../lib/api/types';

export const PLATFORM_LABELS: Record<LmsPlatform, string> = {
  moodle: 'Moodle',
  blackboard: 'Blackboard',
};

export const LOGIN_STATE_LABELS: Record<LoginState, string> = {
  never: 'Not signed in',
  ok: 'Connected',
  expired: 'Session expired',
  error: 'Sign-in failed',
};
```

- [ ] **Step 3: Write failing tests** — `AddUniversityModal.test.tsx` (mock `api` like `CatalogPage.test.tsx` did):

```tsx
describe('AddUniversityModal', () => {
  it('probes, saves, then signs in and shows the connected account', async () => {
    mockedApi.lmsProbe.mockResolvedValue({ baseUrl: 'https://aulavirtual.uv.es' });
    mockedApi.addUniversity.mockResolvedValue(university({ loginState: 'never' }));
    mockedApi.lmsSignIn.mockResolvedValue(university({ loginState: 'ok', accountLabel: 'Ana Pérez' }));

    renderModal();
    fireEvent.change(screen.getByLabelText('Name'), { target: { value: 'UV' } });
    fireEvent.change(screen.getByLabelText('Platform'), { target: { value: 'moodle' } });
    fireEvent.change(screen.getByLabelText('Platform URL'), { target: { value: 'aulavirtual.uv.es/my' } });
    fireEvent.click(screen.getByRole('button', { name: 'Continue' }));

    expect(await screen.findByText('https://aulavirtual.uv.es')).toBeInTheDocument();
    expect(mockedApi.addUniversity).toHaveBeenCalledWith({ name: 'UV', platform: 'moodle', baseUrl: 'aulavirtual.uv.es/my' });

    fireEvent.click(screen.getByRole('button', { name: 'Sign in' }));
    expect(await screen.findByText('Connected as Ana Pérez')).toBeInTheDocument();
  });

  it('shows the probe error inline and does not save', async () => {
    mockedApi.addUniversity.mockRejectedValue({ kind: 'validation', message: 'This URL is not a Moodle site' });
    renderModal();
    fireEvent.change(screen.getByLabelText('Name'), { target: { value: 'UV' } });
    fireEvent.change(screen.getByLabelText('Platform URL'), { target: { value: 'example.com' } });
    fireEvent.click(screen.getByRole('button', { name: 'Continue' }));
    expect(await screen.findByText('This URL is not a Moodle site')).toBeInTheDocument();
  });

  it('keeps the dialog usable after a cancelled sign-in', async () => {
    mockedApi.addUniversity.mockResolvedValue(university({ loginState: 'never' }));
    mockedApi.lmsSignIn.mockRejectedValueOnce({ kind: 'auth', message: 'Sign-in cancelled' });
    renderModal();
    fireEvent.change(screen.getByLabelText('Name'), { target: { value: 'UV' } });
    fireEvent.change(screen.getByLabelText('Platform URL'), { target: { value: 'aulavirtual.uv.es' } });
    fireEvent.click(screen.getByRole('button', { name: 'Continue' }));
    fireEvent.click(await screen.findByRole('button', { name: 'Sign in' }));
    expect(await screen.findByText('Sign-in cancelled')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Sign in' })).toBeEnabled();
  });
});
```

`AddUniversityModal` calls `api.addUniversity` directly (the backend probes); `lmsProbe` is only used if you add a separate "Check URL" affordance — do not add one. Remove the `lmsProbe` mock line from the first test if unused. Provide `university(overrides)` and `renderModal()` helpers in the test file (QueryClientProvider + `<AddUniversityModal open onClose={vi.fn()} />`).

`UniversitiesPage.test.tsx`:

```tsx
describe('UniversitiesPage', () => {
  it('lists universities with their connection state', async () => {
    mockedApi.getUniversities.mockResolvedValue([
      university({ id: 1, name: 'UV', loginState: 'ok', accountLabel: 'Ana Pérez' }),
      university({ id: 2, name: 'UAH', platform: 'blackboard', baseUrl: 'https://uah.blackboard.com', loginState: 'expired' }),
    ]);
    renderPage();
    expect(await screen.findByText('Connected as Ana Pérez')).toBeInTheDocument();
    expect(screen.getByText('Session expired')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Sign in to UAH' })).toBeInTheDocument();
  });

  it('shows an empty state with the add action', async () => {
    mockedApi.getUniversities.mockResolvedValue([]);
    renderPage();
    expect(await screen.findByText('No universities yet')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Add university' })).toBeInTheDocument();
  });

  it('checks the connection and shows the error inline', async () => {
    mockedApi.getUniversities.mockResolvedValue([university({ id: 1, name: 'UV', loginState: 'ok' })]);
    mockedApi.lmsCheckSession.mockRejectedValue({ kind: 'auth', message: 'UV needs you to sign in again' });
    renderPage();
    fireEvent.click(await screen.findByRole('button', { name: 'Check connection to UV' }));
    expect(await screen.findByText('UV needs you to sign in again')).toBeInTheDocument();
  });

  it('confirms before deleting', async () => {
    mockedApi.getUniversities.mockResolvedValue([university({ id: 1, name: 'UV' })]);
    mockedApi.removeUniversity.mockResolvedValue(undefined);
    renderPage();
    fireEvent.click(await screen.findByRole('button', { name: 'Delete UV' }));
    expect(screen.getByText('Deletes this university, its courses and your saved sign-in.')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Delete' }));
    await waitFor(() => expect(mockedApi.removeUniversity).toHaveBeenCalledWith(1));
  });
});
```

Run: `npm test -- src/features/universities` → Expected: FAIL (components missing).

- [ ] **Step 4: Implement the components.** Requirements (match the existing pages' Tailwind style; reuse `Modal`, `ConfirmDialog`, `Field`, `TextInput`, `Select`, `ErrorMessage`; use `toCommandError` for every rejected mutation; invalidate `queryKeys.universities` and `queryKeys.catalog` after add/rename/remove/sign-in/sign-out/check):
  - **`UniversitiesPage`**: heading "Universities", "Add university" button, `useQuery({ queryKey: queryKeys.universities, queryFn: api.getUniversities })`; empty state text "No universities yet" with the add button; otherwise one `UniversityCard` per university.
  - **`UniversityCard`**: name; `PLATFORM_LABELS[platform]` · `baseUrl`; status line: `Connected as {accountLabel}` when `loginState === 'ok'` and a label exists, else `LOGIN_STATE_LABELS[loginState]`; `{courses.tracked} of {courses.total} courses tracked` only when `courses.total > 0`. Buttons with accessible names including the university name (`aria-label`): "Sign in to {name}" (shown when `loginState !== 'ok'`; label text "Sign in", pending text "Waiting for sign-in…"), "Check connection to {name}" (when `loginState !== 'never'`), "Sign out of {name}" (when `loginState !== 'never'`), "Rename {name}", "Delete {name}". Inline `ErrorMessage` for the last failed action of that card. Delete opens `ConfirmDialog` with title "Delete university" and message "Deletes this university, its courses and your saved sign-in.".
  - **`AddUniversityModal`** (`open`, `onClose`): step 1 fields Name (`TextInput`, id `university-name`), Platform (`Select`, id `university-platform`, options Moodle/Blackboard, default `moodle`), Platform URL (`TextInput`, id `university-url`, placeholder `https://aulavirtual.uv.es`, hint "The address you use to open the platform in a browser."). "Continue" → `api.addUniversity` (pending text "Checking…"); errors inline. Step 2 shows the saved `baseUrl`, the text "Sign in with your university account in the window that opens.", a "Sign in" button (pending "Waiting for sign-in…") → `api.lmsSignIn(id)`; on success shows "Connected as {accountLabel}" and a "Done" button that closes. Errors inline; the button stays enabled for retry. Closing at step 2 keeps the saved university (the page lists it as "Not signed in").
  - **`RenameUniversityModal`**: single Name field, Save/Cancel, inline error.
- Then route + nav: `App.tsx` `<Route path="/universities" element={<UniversitiesPage />} />` (remove the catalog route and import); `Layout.tsx` nav item replaced; update `Layout.test.tsx` `NAV_LABELS` (`'University catalog'` → `'Universities'`). Delete `src/features/catalog/`.

- [ ] **Step 5: Run tests and gates**

Run: `npm run typecheck && npm run lint && npm test` → Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add -A src
git commit -m "feat(ui): add Universities page and remove the manual catalog"
```

---

### Task 7: Live tests and the M1 validation checklist

**Files:**
- Create: `crates/infra/tests/lms_live.rs`
- Modify: `docs/testing.md` (new section "Phase 2 · M1 sign-in gate")

**Interfaces:**
- Consumes: Task 3 (`DefaultLmsClientFactory`), Task 1 (`LmsUniversity`), `KeyringSecretStore`.

- [ ] **Step 1: Live test** (ignored by default; reads the credential the app stored):

```rust
//! Live checks against a real platform with the credential stored by the app.
//! Run after signing in through the app:
//!   $env:LMS_LIVE_UNIVERSITY_ID="1"; $env:LMS_LIVE_PLATFORM="moodle"; $env:LMS_LIVE_BASE_URL="https://aulavirtual.uv.es"
//!   cargo test -p academic-infra --test lms_live -- --ignored --nocapture
//! Prints only counts and the display name, never credentials or course data.

use std::sync::Arc;

use academic_core::domain::{LmsPlatform, LmsUniversity, LoginState};
use academic_core::ports::LmsClientFactory;
use academic_infra::factory::http_client;
use academic_infra::lms::factory::DefaultLmsClientFactory;
use academic_infra::secrets::KeyringSecretStore;
use chrono::Utc;

fn university_from_env() -> LmsUniversity {
    let platform = match std::env::var("LMS_LIVE_PLATFORM").expect("LMS_LIVE_PLATFORM").as_str() {
        "moodle" => LmsPlatform::Moodle,
        "blackboard" => LmsPlatform::Blackboard,
        other => panic!("unknown platform {other}"),
    };
    LmsUniversity {
        id: std::env::var("LMS_LIVE_UNIVERSITY_ID").expect("LMS_LIVE_UNIVERSITY_ID").parse().unwrap(),
        name: "live".into(),
        platform,
        base_url: std::env::var("LMS_LIVE_BASE_URL").expect("LMS_LIVE_BASE_URL"),
        login_state: LoginState::Ok,
        account_label: None,
        last_sync_at: None,
        last_sync_outcome: None,
        last_sync_error: None,
        created_at: Utc::now(),
    }
}

#[tokio::test]
#[ignore = "needs a real platform and a credential stored by the app"]
async fn live_verify_and_list_courses() {
    let factory = DefaultLmsClientFactory::new(Arc::new(KeyringSecretStore), http_client());
    let client = factory.create(&university_from_env()).expect("stored credential");
    let identity = client.verify().await.expect("verify");
    println!("verified as: {}", identity.display_name);
    let courses = client.list_courses().await.expect("list_courses");
    println!("courses: {}", courses.len());
    assert!(!courses.is_empty());
}
```

- [ ] **Step 2: Checklist** — append to `docs/testing.md`:

```markdown
## Phase 2 · M1 sign-in gate (manual, real accounts)

Run `npm run tauri dev`. Record date, result and observations for each line in the PR/merge notes.

1. Universities → Add university → "UV", Moodle, `aulavirtual.uv.es` → Continue shows `https://aulavirtual.uv.es`.
2. Sign in → the UV login opens in a separate window → after login the window closes by itself and the dialog shows "Connected as …".
3. Repeat with "UAH", Blackboard, `uah.blackboard.com` (Microsoft login). Same result.
4. Wrong platform: add `aulavirtual.uv.es` as Blackboard → "This URL is not a Blackboard Learn site". Add `aulavirtual.uv.es` again as Moodle → "This platform is already added".
5. Close the sign-in window before finishing → "Sign-in cancelled"; signing in again works.
6. Restart the app → both universities still "Connected"; "Check connection" on each succeeds.
7. Live test for each university (see `crates/infra/tests/lms_live.rs`) prints the display name and a course count > 0.
8. Silent renewal: delete the stored credential (`cmdkey /list | findstr university` then `cmdkey /delete:<target>`), press "Check connection": the session is renewed without a visible window (no login prompt) and the card shows "Connected". If the IdP requires interaction, the card shows "Session expired" and "Sign in" works — record which happened for UV and UAH.
9. Leave the app for several hours (or overnight), then "Check connection" on UAH: record whether the Blackboard session had expired and whether renewal was silent. Record how long the Moodle token kept working.
10. Sign out → card shows "Not signed in"; the credential is gone from `cmdkey /list`. Delete → the university disappears.
11. Logs (`%APPDATA%/dev.eaybar.academic-assistant/logs`): no tokens, cookie values or login-window URLs; only university ids, stages and error kinds.
12. Credential size: step 3 succeeded without "too large to store securely" (if it failed, note the cookie names logged).
```

- [ ] **Step 3: Run gates; commit**

```bash
git add crates/infra/tests/lms_live.rs docs/testing.md
git commit -m "test: add live LMS checks and the M1 sign-in checklist"
```

---

## After M1

The planner merges the task branches into `phase-2-lms-tracker`, runs the full gates, and hands the checklist to the user. **M2–M5 are planned only after the user reports the checklist results**, because the capture mechanism, token/session lifetimes and real API responses decide details of the next milestones (spec §14, §18.6).
