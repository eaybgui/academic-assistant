# Android App Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship an installable Android app (tablet and phone) with today's Dashboard, Calendar and Universities sections and no email or AI, without changing the desktop app.

**Architecture:**
- **Same repository, same crates and React code, built with Tauri 2 for Android.**
- **`#[cfg(target_os = "android")]` selects Android-only code,** and target-specific Cargo dependencies keep email and AI out of the Android build. The desktop's dependency graph and behaviour do not change.
- **One local Tauri plugin holds all Kotlin code** (secure storage and the sign-in screen). Rust reaches it through `PluginHandle::run_mobile_plugin`.
- **A throwaway sign-in test comes first.** The product sign-in task (Task 5) is written from its results.

**Tech Stack:** Tauri 2.12 (Android), Rust, Kotlin, React 19, TypeScript, Tailwind v4, Vitest.

**Spec:** `docs/superpowers/specs/2026-11-11-android-app-design.md`. It is the binding authority. Read §4 (anti-goals), §5 (user decisions), §7 and §9.

## Global Constraints

- **Rules:** follow `AGENTS.md`: only your task, no unrelated refactors, report every deviation. New dependencies are only those this plan names.
- **Public repo (AGENTS.md §11.2):** tests and docs use invented data only. Tokens, cookies, account names, course names and anything captured from a real sign-in must never be committed, logged or written to a report. UV's and UAH's public institutional addresses are accepted.
- **Desktop unchanged (spec §4):** every task leaves these passing on Windows with no change in behaviour: `npm run typecheck`, `npm run lint`, `npm test`, `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`.
- **Android selection:** Rust uses `#[cfg(target_os = "android")]` / `#[cfg(not(target_os = "android"))]` everywhere (`crates/infra` does not get Tauri's `mobile` alias). The frontend uses `IS_ANDROID` from `src/config/platform.ts`.
- **No password is ever stored or read by the app.** Tokens and cookies live only in the Keystore-backed store, never in SQLite, plain files or ordinary shared preferences. Never log them.
- **Android check:** `npm run tauri -- android build --debug --apk --target x86_64` for the emulator, `--target aarch64` for the tablet. The output APK is under `src-tauri/gen/android/app/build/outputs/apk/`. Clippy on the Android target must produce no new warnings in code this plan adds.
- **Toolchain:** the Android SDK, NDK, Java and Rust Android targets are installed separately. `JAVA_HOME`, `ANDROID_HOME` and `NDK_HOME` must be set. If they aren't, stop and report it.
- **Git:** each task works on `feature/p4-t{N}-{slug}` from `phase-4-android`, except Task 1 (`spike/android-signin`). Commit there, and never merge or push. Trailer: `Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>`.
- **UI:** English, sentence case, existing Tailwind look and shared classes in `src/components/styles.ts`.

## Review Focus

1. **Android Back button with a dialog open** closes the dialog and stays in the app; with nothing open it goes to the previous screen, and only exits from the first screen. Pinned in Task 4.
2. **A narrow desktop window** keeps today's sidebar and never shows the bottom tab bar. Pinned in Task 4 (`IS_ANDROID` false).
3. **A phone-width screen with long titles or several universities** has no sideways scrolling; tabs and controls wrap. Pinned in Task 4 and the Task 6 checklist.
4. **Stored credentials that can no longer be decrypted** (key removed, app data restored) read as "not signed in", so the banner appears, and the app does not crash. Pinned in Task 3.
5. **Backing out of the sign-in screen** returns to the app with sign-in cancelled and the university's state unchanged. Pinned in Task 5.

---

### Task 1: Sign-in test on the real tablet (throwaway, the gate)

**Purpose:** prove spec §7.1 before any product work. No TDD and no review of code quality: this code is **not merged**. Branch `spike/android-signin` from `phase-4-android`.

**Files:** anything needed on the spike branch (`src-tauri/gen/android`, a local Kotlin plugin, a minimal debug screen). Keep product code unmodified where possible.

**The test must establish, on the user's tablet:**

1. UV (Moodle): an Android `WebView` screen opens `moodle_launch_url(...)`, the university login completes, and `shouldOverrideUrlLoading` captures the `moodlemobile://token=…` redirect. `decode_launch_token` accepts it.
2. UAH (Blackboard): the Microsoft Entra login completes in the same screen; after `is_blackboard_signed_in` is true, `CookieManager.getCookie(baseUrl)` returns the session cookies. Note that `getCookie` does expose HttpOnly cookies but no domains.
3. One real request from Rust succeeds for each platform (Moodle `core_webservice_get_site_info`, Blackboard `users/me`) with those credentials, including TLS. Record whether `reqwest`'s `rustls` needs Android initialisation.
4. The credential is encrypted with an Android Keystore AES-GCM key, stored privately, and read back after force-stopping and reopening the app.
5. Quiet renewal, not blocking: after restart, a sign-in screen with the WebView's remembered session completes with no interaction, invisibly if possible. Record the result per platform.

**Also record:** the tablet's Android version (API level) and CPU architecture (`adb shell getprop ro.build.version.sdk`, `ro.product.cpu.abi`); what `tauri android init` does with the hyphen in identifier `dev.eaybar.academic-assistant`; whether `rusqlite` bundled and the TLS crypto backend cross-compile on this PC and with which environment; whether the CSP in `tauri.conf.json` works on Android or needs a `tauri.android.conf.json` override; whether `openUrl` opens a browser; default Back button behaviour.

- [ ] **Step 1:** Build a debug APK (`--target aarch64`) and install with `adb install -r`. Use the emulator first for anything not needing a real login.
- [ ] **Step 2:** The sub-agent reports the APK path and exact tap-through steps. **The user performs the real sign-ins on the tablet**; credentials are typed only by the user. The debug screen shows only yes/no results and lengths, never credential values.
- [ ] **Step 3:** Write the results to `.superpowers/android-gate-report.md` (git-ignored; no real tokens, cookies, course names or account names), one PASS or FAIL per check with the approach that worked.
- [ ] **Step 4:** Commit only on `spike/android-signin`. Do not push.

**Gate:** if any of checks 1–4 fails, stop and bring the options to the user. If they pass, the planner writes Task 5's steps into this plan from the report, updates spec §7 where the result differs, and execution continues with Task 2.

---

### Task 2: Android build without email or AI

**Files:**
- Modify: `crates/infra/Cargo.toml`: move `keyring`, `async-imap`, `tokio-rustls`, `webpki-roots`, `mail-parser` and any other dependency only email code uses to `[target.'cfg(not(target_os = "android"))'.dependencies]`.
- Modify: `crates/infra/src/lib.rs`: `graph`, `imap`, `jev`, `ollama` under `#[cfg(not(target_os = "android"))]`.
- Modify: `crates/infra/src/factory.rs`: `http_client()` stays available on every target; `DefaultEmailProviderFactory` and `DefaultModelProvider` are desktop-only.
- Modify: `crates/infra/src/secrets.rs`: `KeyringSecretStore` and its keyring helpers desktop-only; `MAX_SECRET_BYTES` and `InMemorySecretStore` stay on every target.
- Modify in `src-tauri/`:
  - `src/lib.rs`, `src/commands/mod.rs` (`accounts`, `catalog`, `results`, `runs`, `settings` desktop-only)
  - `src/dto/mod.rs` (same five)
  - `src/state.rs`, `src/setup.rs`, `src/validation.rs`
  - `account_status`, `observer` modules desktop-only
- Create: `src-tauri/src/lms_auth/android.rs` (temporary stub, replaced in Task 5); gate `capture`/`window` as Task 5 needs.
- Create: `src-tauri/tauri.android.conf.json`; plus CSP override only if Task 1 reported it's needed.
- Create: `src-tauri/gen/android/` via `npm run tauri -- android init` (commit it; the generated `.gitignore` covers build output and `local.properties`).
- Modify: `package.json`: add `"build:android": "tsc -b && vite build --mode android"`.

**Interfaces:**
- Consumes: Task 1 report (identifier handling, cross-compile environment, CSP).
- Produces, for Tasks 3 and 5:

```rust
// src-tauri/src/lms_auth/mod.rs exposes the same names on both targets.
// Android signatures (stubbed here, implemented in Task 5):
pub async fn capture_credential(app: &AppHandle, university: &LmsUniversity, mode: LoginMode) -> Result<String, LmsError>;
pub fn close_login_window(app: &AppHandle, id: UniversityId);
pub fn close_all_login_windows(app: &AppHandle);
pub enum LoginMode { Interactive, Silent }
pub struct TauriLmsAuthenticator; // new(app, factory, secrets), implements LmsAuthenticator
```

```rust
// src-tauri/src/lib.rs: one list of LMS commands, no duplication.
macro_rules! lms_handler {
    ($($email:tt)*) => { tauri::generate_handler![ $($email)* universities::lms_probe, /* …all LMS, tracker and personal commands… */ ] };
}
// desktop: .invoke_handler(lms_handler!(runs::check_now, …, settings::test_ai_provider,))
// android: .invoke_handler(lms_handler!())
```

`AppState` and `Collaborators` keep on Android only: `universities`, `courses`, `items`, `personal_tasks`, `secrets`, `http`, `clock`, `lms_factory`, `lms_authenticator`, `lms_observer`, `lms_sync`. The rest are desktop-only, including `runner`. `setup.rs` skips `mark_interrupted` on Android. Stubs: the `lms_auth` Android functions return `LmsError::Protocol("Sign-in is not available yet")`; on Android, `setup.rs` uses `academic_infra::secrets::InMemorySecretStore` until Task 3.

`tauri.android.conf.json`:

```json
{ "build": { "beforeBuildCommand": "npm run build:android" } }
```

- [ ] **Step 1:** Run the Windows gates to record the baseline.
- [ ] **Step 2:** Run the Android check. Expect FAIL (no Android project or `keyring`/email code on Android).
- [ ] **Step 3:** Run `android init`, apply the exclusions and stubs above.
- [ ] **Step 4:** Run the Android check for `x86_64`, then `aarch64`. Expect a built APK. Install on the emulator: the app opens to the Dashboard first-run card.
- [ ] **Step 5:** Confirm no email or AI code is reachable: `cargo tree --target aarch64-linux-android -p academic-infra` shows no `async-imap` or `keyring`, and every command in the Android branch of `invoke_handler` is LMS, tracker or personal.
- [ ] **Step 6:** Run all Windows gates. Expect the same results as Step 1, plus no change to `src-tauri/tauri.conf.json`.
- [ ] **Step 7:** Commit: `feat(android): build the app for Android without email and AI`.

---

### Task 3: Secure storage plugin

**Files:**
- Create: `crates/tauri-plugin-academic-android/` (workspace member; a local Tauri plugin with an `android/` Gradle module and a `build.rs` using `tauri_plugin::Builder`).
  - `src/lib.rs`: `pub fn init<R: Runtime>() -> TauriPlugin<R>`, registering Kotlin class `AcademicPlugin` in package `dev.eaybar.academicassistant.plugin`, and exposing `handle(app) -> PluginHandle<R>`.
  - `android/src/main/java/.../AcademicPlugin.kt`: commands `secretGet`, `secretSet`, `secretDelete`.
  - `android/src/main/java/.../SecretBox.kt`: pure AES-GCM.
  - `android/src/test/java/.../SecretBoxTest.kt`.
- Create: `src-tauri/src/android_secrets.rs` (`#[cfg(target_os = "android")]`).
- Modify: `src-tauri/Cargo.toml` (the plugin only under `[target.'cfg(target_os = "android")'.dependencies]`), `src-tauri/src/lib.rs`, `src-tauri/src/setup.rs`.

**Interfaces:**
- Consumes: Task 2 (`setup.rs` Android branch).
- Produces:

```rust
pub struct AndroidSecretStore<R: Runtime> { /* PluginHandle<R> */ }
impl<R: Runtime> SecretStore for AndroidSecretStore<R> { get / set / delete }
```

```kotlin
class SecretBox(private val key: SecretKey) {
    fun seal(plain: String): String      // base64(12-byte random IV + AES-256-GCM ciphertext+tag)
    fun open(sealed: String): String?    // null on any failure (wrong key, tampered, malformed)
}
```

Key: alias `academic_assistant_secrets`, AES-256-GCM, created in `AndroidKeyStore` on first use. Sealed values live in app-private `Context.MODE_PRIVATE` preferences file `academic_secrets`, keyed by the secret key. `secretGet` returns `null` when the entry is missing **or** `open` returns null, and on null from `open` removes the bad entry (Review Focus 4). Values up to at least `MAX_SECRET_BYTES` must work; the store itself imposes no smaller limit. Do not add Jetpack Security.

- [ ] **Step 1: Write `SecretBoxTest`** (JVM unit test using a generated `SecretKeySpec`):
  - `seal_then_open_round_trips` with an accented string and a 4096-character string.
  - `seal_uses_a_fresh_iv`: two seals of the same text differ.
  - `open_returns_null_for_a_wrong_key`, `open_returns_null_for_tampered_data`, `open_returns_null_for_malformed_base64`.
- [ ] **Step 2:** Run the Gradle unit tests for the plugin module. Expect FAIL.
- [ ] **Step 3:** Implement `SecretBox`, the plugin commands, `AndroidSecretStore`, and use it in `setup.rs` on Android.
- [ ] **Step 4:** Unit tests pass. Android check builds. On the emulator, with a temporary debug log of booleans only (removed before commit): set, restart, get returns the value; delete, get returns none.
- [ ] **Step 5:** Windows gates pass; `cargo tree -p academic-assistant` on Windows does not include the plugin.
- [ ] **Step 6:** Commit: `feat(android): keep credentials in Keystore-backed storage`.

---

### Task 4: Android shell: bottom tab bar, phone layout, Back button

**Files:**
- Create: `src/config/platform.ts`, `src/components/BottomTabBar.tsx`, `src/components/BottomTabBar.test.tsx`, `src/features/shell/useAndroidBack.ts`, `src/features/shell/useAndroidBack.test.tsx`
- Create: `src-tauri/capabilities/android.json` (`"platforms": ["android"]`, `"windows": ["main"]`, the permission `onBackButtonPress` needs).
- Modify: `src/config/features.ts`, `src/components/Layout.tsx`, `src/components/Layout.test.tsx`, `index.html` (add `viewport-fit=cover`), and the page files below as needed for phone width.

**Interfaces:**
- Consumes: `visibleNavItems()` / `NavItemConfig` in `Layout.tsx` (export them so the tab bar shares the one list).
- Produces:

```ts
// src/config/platform.ts: follows features.ts's style
export const IS_ANDROID: boolean = import.meta.env.MODE === 'android';
// src/config/features.ts
export const EMAIL_FEATURES_ENABLED: boolean = import.meta.env.VITE_EMAIL_FEATURES === 'true' && !IS_ANDROID;
// BottomTabBar.tsx
export default function BottomTabBar(): JSX.Element; // <nav aria-label="Sections">, one NavLink per visible item
// useAndroidBack.ts
export function useAndroidBack(): void; // mounted once in Layout, no-op unless IS_ANDROID
```

Requirements:
- **`IS_ANDROID` false:** `Layout` renders exactly today's markup and classes (Review Focus 2).
- **`IS_ANDROID` true:**
  - The sidebar has `hidden md:block`.
  - `BottomTabBar` renders in a `fixed inset-x-0 bottom-0 md:hidden` bar with `pb-[env(safe-area-inset-bottom)]`, three equal items (icon and label, the active one highlighted).
  - `<main>` uses `px-4 pb-24 md:px-8 md:pb-6`, `py-6` otherwise as today.
  - The tablet (`md` and up) shows the sidebar.
- **Back (`useAndroidBack`, via `onBackButtonPress` from `@tauri-apps/api/app`):** if any `dialog[open]` exists, dispatch a bubbling `keydown` with `key: 'Escape'` on the last one in document order (the topmost), so `Modal`'s existing Escape path runs `onClose`; else if `canGoBack`, `window.history.back()`; else exit the app with `getCurrentWindow().close()`. If `close()` doesn't leave the app on the emulator, stop and report it. (Review Focus 1)
- **Phone-width reflow, under `IS_ANDROID` or additive `flex-wrap`/`min-w-0` that does not change desktop rendering:** Dashboard header and its button row wrap (`TrackerPage.tsx`), university tabs wrap, `ItemRow`/`PersonalTaskRow` content stays within the screen, universities cards and modal forms fit. Tap targets are at least `h-9` for the bottom bar and the calendar's nav buttons on Android.
- **Touch:** no code path may require hover. Dot titles stay (`title`/`aria-label`); the day list is the touch equivalent (spec §9).

- [ ] **Step 1: Write the failing tests** (use `vi.mock('../config/platform', …)` for both values):
  - `Layout.test.tsx`: `IS_ANDROID` false → no element with `aria-label="Sections"`, and the sidebar lacks `hidden`; `IS_ANDROID` true → the tab bar has exactly `['Dashboard', 'Calendar', 'Universities']` and `<main>` has `pb-24`.
  - `BottomTabBar.test.tsx`: the active route's link has `aria-current="page"`; no email section appears.
  - `features` test: with `MODE` android and `VITE_EMAIL_FEATURES` `'true'` (`vi.stubEnv` plus dynamic import), `EMAIL_FEATURES_ENABLED` is false.
  - `useAndroidBack.test.tsx` (mock the Tauri API and capture the callback):
    - an open dialog gets closed and `history.back` is not called;
    - no dialog and `canGoBack: true` calls `history.back`;
    - no dialog and `canGoBack: false` calls `close`;
    - with `IS_ANDROID` false, `onBackButtonPress` is never registered.
- [ ] **Step 2:** `npm test -- src/components src/features/shell src/config`. Expect FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Tests pass; Windows gates pass. Android check, then on the emulator at phone size (pixel profile) and tablet size: tabs switch sections, no sideways scroll on Dashboard, Calendar and Universities, Back closes a dialog.
- [ ] **Step 5:** Commit: `feat(android): add the phone layout and Back button handling`.

---

### Task 5: Android sign-in (written from the gate results)

**Gate results (2026-11-11, Xiaomi Pad 7, Android 16 / API 36, arm64-v8a).** All checks passed:
- **Moodle:** the token was captured, decoded, stored and used.
- **Blackboard:** 1,109 bytes of cookies via `CookieManager.getCookie`; a real request succeeded.
- **Restart:** both credentials survived it.
- **Quiet renewal:** an invisible `WebView` using the remembered login session succeeded silently for **both** platforms.

The approach that worked was the spike's `SignInActivity`: a separate `ComponentActivity` with a `WebView` (JavaScript and DOM storage on, third-party cookies accepted). It intercepts `moodlemobile://` in `shouldOverrideUrlLoading`. For Blackboard it watches `doUpdateVisitedHistory` for the `is_blackboard_signed_in` rule, then flushes and reads `CookieManager.getCookie(baseUrl)`. Back delivers "cancelled". Reference: `git show spike/android-signin:src-tauri/gen/android/app/src/main/java/dev/eaybar/academic_assistant/SignInActivity.kt`. Rebuild it to this task's contract; never merge the spike.

**Files:**
- Create: `crates/tauri-plugin-academic-android/android/src/main/java/.../SignInActivity.kt`, declared in the plugin's `AndroidManifest.xml`.
- Modify: `AcademicPlugin.kt` (commands below).
- Create: `src-tauri/src/lms_auth/sign_in_result.rs` (target-independent, unit-tested on Windows).
- Replace: `src-tauri/src/lms_auth/android.rs` (the Task 2 stub).
- Modify: `src-tauri/src/lms_auth/capture.rs`, `src-tauri/src/lms_auth/mod.rs`, `src-tauri/src/commands/universities.rs` (sign-out hook), and `src-tauri/src/lms_auth/window.rs` (only to share the timeout constants).

**Interfaces:**
- Consumes: Task 2's `lms_auth` names, Task 3's plugin and `AndroidSecretStore`, and the pure helpers in `capture.rs` (`moodle_launch_url`, `blackboard_start_url`, `moodle_token_payload`, `is_blackboard_signed_in`, `encode_session_cookies`, `RawCookie`), plus `decode_launch_token`.
- Produces:

```kotlin
// AcademicPlugin commands (Invoke payloads are JSON objects):
signIn({ url: String, interceptScheme: String?, doneUrlPrefix: String?, cookieUrl: String?, silent: Boolean, timeoutMs: Long })
  -> { status: "redirect" | "signedIn" | "cancelled" | "timeout", url: String?, cookieHeader: String? }
cancelSignIn() -> {}            // finishes an open SignInActivity, which then resolves "cancelled"
clearSignInData() -> {}         // CookieManager.removeAllCookies + WebStorage.deleteAllData
```

```rust
// src-tauri/src/lms_auth/sign_in_result.rs (no cfg: tested on Windows)
pub enum SignInOutcome { Redirect(String), SignedIn(String /* cookie header */), Cancelled, TimedOut }
pub fn parse_cookie_header(header: &str) -> Vec<RawCookie>;   // "a=b; c=d=e" -> [a=b, c=d=e], domain None, skips empty/invalid parts
pub fn credential_from_outcome(outcome: SignInOutcome, platform: LmsPlatform, base_url: &str, passport: &str, mode: LoginMode)
    -> Result<String, LmsError>;
// Redirect + Moodle -> moodle_token_payload + decode_launch_token; SignedIn + Blackboard -> parse_cookie_header + encode_session_cookies(host of base_url);
// Cancelled / TimedOut / mismatched -> exactly the LmsError values the desktop's capture_credential returns for a closed window / timeout in that mode.
pub fn blackboard_done_prefix(base_url: &str) -> String;     // "{base}/ultra/" — is_blackboard_signed_in must use it too (one source of the rule)
```

`lms_auth` adds, on both targets:

```rust
pub fn forget_sign_in(app: &AppHandle);
// desktop: does nothing (desktop behaviour unchanged); android: plugin clearSignInData
```

`lms_sign_out` gains an `app: AppHandle` parameter and calls it after the existing sign-out succeeds.

Requirements:
- **Sign-in parameters:**
  - Moodle: `interceptScheme = "moodlemobile"`.
  - Blackboard: `doneUrlPrefix = blackboard_done_prefix(base)` and `cookieUrl = base`.
  - `timeoutMs` comes from the desktop's `INTERACTIVE_TIMEOUT` / `SILENT_TIMEOUT`. Move the constants to `mod.rs` so both targets share them.
  - `silent` is `mode == LoginMode::Silent`, which makes the `WebView` invisible.
- **Threading:** the plugin call is async (`run_mobile_plugin_async`) and never blocks the main thread.
- **One sign-in at a time per app:** a second `signIn` while one is open is rejected the same way the desktop refuses a second window for the same university (same `LmsError`).
- **`close_login_window` / `close_all_login_windows`:** call `cancelSignIn`.
- **Cancel (Review Focus 5):** Back, closing the screen or `cancelSignIn` gives "cancelled". The university's login state and stored secret don't change, as on the desktop.
- **Sign out:** clears all `WebView` cookies and storage (Ruling R9). The other university's stored credential stays valid; only its quiet renewal may need one interactive sign-in afterwards.
- **No logging or UI exposure:** nothing logs or returns a token, cookie or URL query string. `SignInActivity` shows only the page and a "Sign in to {name}" title bar.

- [ ] **Step 1: Write the failing Rust tests** in `sign_in_result.rs` (invented values only):
  - `parse_cookie_header_splits_and_trims`: `"a=1; b=2=3; ;c"` gives `[("a","1"), ("b","2=3")]`.
  - `blackboard_signed_in_outcome_encodes_the_session`: `SignedIn("BbRouter=x; JSESSIONID=y")` for `https://campus.demo.example` gives `Ok(credential)` equal to `encode_session_cookies` of the same two cookies.
  - `moodle_redirect_outcome_decodes_the_token`: a `moodlemobile://token=` URL built with the test helpers already used in `capture.rs`/`moodle.rs` tests gives the decoded token.
  - `cancelled_and_timed_out_map_like_the_desktop`: they give the same `LmsError` kinds/messages as `window.rs` for Interactive and Silent.
  - `mismatched_outcome_is_a_protocol_error`: a `Redirect` for Blackboard gives `LmsError::Protocol`.
  - `is_blackboard_signed_in_uses_the_done_prefix`: the existing `capture.rs` tests still pass, plus one asserting `blackboard_done_prefix("https://x.example") == "https://x.example/ultra/"`.
- [ ] **Step 2:** `cargo test -p academic-assistant sign_in_result`. Expect FAIL.
- [ ] **Step 3:** Implement the Rust side, `SignInActivity`, the plugin commands and the sign-out hook.
- [ ] **Step 4:** Windows gates pass and the desktop sign-in tests are unchanged. Android check builds. On the emulator: start a sign-in to a university and press Back, and the university stays unchanged. Silent mode with no remembered session returns the timeout error within `SILENT_TIMEOUT`.
- [ ] **Step 5: Tablet (user).** Install the debug APK. Add UV and UAH, sign in to each, sync, restart the app, sync again. Sign out of UV and check that UAH still syncs. Record PASS or FAIL in the report; no secrets.
- [ ] **Step 6:** Commit: `feat(android): sign in to Moodle and Blackboard on Android`.

---

### Task 6: Installable APK, docs and acceptance checklist

**Files:**
- Modify: `src-tauri/gen/android/app/build.gradle.kts`: a release `signingConfig` read from git-ignored `src-tauri/gen/android/keystore.properties`; if that file is absent, the release build is left unsigned and says so.
- Modify: the generated `.gitignore` or root `.gitignore` to ignore `keystore.properties` and `*.jks`.
- Modify: `docs/development.md` (Android setup, build and install commands, signing), `docs/testing.md` (new "Android acceptance checklist"), `docs/infra/secure-storage.md` (Android store), `docs/architecture.md` (Android composition), `README.md` (Android status), and the spec (append "§11 As built" with the gate results and deviations; invented values only).

Requirements:
- The release key is generated with `keytool` and stored **outside the repository** (`%USERPROFILE%\.academic-assistant\android-release.jks`), and its passwords are only in `keystore.properties`.
- Before committing, confirm with `git status` and `git check-ignore` that neither is tracked.
- The checklist covers, on the tablet:
  - install the APK; only Dashboard, Calendar and Universities exist;
  - add UV and UAH, sign in to each, cancel a sign-in with Back;
  - choose courses, sync;
  - Dashboard tabs, grouping, collapse, done, hide, note, details, Open in platform;
  - personal tasks; Calendar dots, day list, add;
  - restart keeps data and sign-in;
  - sign out; expired session shows the banner;
  - airplane mode shows synced data, and Sync shows an error;
  - Back button behaviour;
  - on the emulator at phone size: the tab bar and no sideways scrolling.

- [ ] **Step 1:** Generate the key, wire the signing config, build `npm run tauri -- android build --apk --target aarch64`.
- [ ] **Step 2:** Verify the signature (`apksigner verify`) and install on the emulator or tablet.
- [ ] **Step 3:** Write the docs. `git diff --check`; grep the diff for anything real-looking.
- [ ] **Step 4:** Windows gates pass.
- [ ] **Step 5:** Commit: `docs(android): add build instructions and the acceptance checklist` (plus the Gradle change, as `feat(android): sign the release APK`).

---

## Task order

`T1 (gate) → planner writes T5 → T2 → T3 → T4 → T5 → T6`, strictly sequential.
