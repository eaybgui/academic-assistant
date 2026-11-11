# Android app: design

Date: 2026-11-11 · Status: draft for approval · Builds on `main` (desktop app, LMS tracker, calendar and personal tasks).

## 1. Intent

The user wants the app they already use on the desktop on their Android tablet, and on phone-sized screens too: "exactly the same I can see now, the new calendar section and the dashboard, with all the functionalities that exist today, except for the email related ones."

Success: the user installs the app file on the tablet, adds UV (Moodle) and UAH (Blackboard), signs in through each university's own login page, chooses courses, syncs, and then uses the Dashboard and Calendar as on the desktop.

## 2. Goals

- An Android app with the same **Dashboard**, **Calendar** and **Universities** sections as the desktop, with their current behaviour:
  - **Dashboard:** All and per-university tabs, grouping by time or status, collapsible sections, badges and chips, done, hide, notes, details, "Add task", and manual Sync.
  - **Calendar:** month grid with status dots, selected-day list, adding tasks.
  - **Universities:** add a university, sign in, choose courses, sync, rename, delete.
  - Personal tasks work as on the desktop.
- It works on tablet-sized and phone-sized screens.
- The desktop app keeps working exactly as it does today.

## 3. Non-goals

- **No email or AI.** Announcements, Email accounts, Email runs and Settings are absent. Everything on the Settings page today is AI provider, Microsoft sign-in or email filters, so there is no Settings section on Android.
- **No iOS.**
- **No Google Play publishing** for now. The user installs the app file (APK) themselves. Play Store is a possible later step.
- **No data sharing** between desktop and Android. Each device has its own universities, tasks, notes and done marks.
- **No new features:** no notifications or reminders, no background or automatic sync.

## 4. Anti-goals: failures even if it works

- **Desktop regressions.** Any change in behaviour on Windows, or breaking its build or tests, is a failure.
- **Passwords stored or seen by the app.** As on the desktop, the user signs in on the university's own page and the app keeps only a scoped token or session, in the platform's secure storage. Plain-text token or cookie storage (a file, SQLite, ordinary shared preferences) is a failure.
- **A forked copy of the app.** Duplicating the core logic, the LMS clients or the React screens into a separate Android codebase is a failure. Platform-specific code stays limited to what truly differs.
- **Email or AI code reachable on Android.** It must not be shipped as hidden screens or callable commands.
- **Real data in the repo.** The repository is public (AGENTS.md §11.2), so tests and docs use invented data only. The public institutional addresses of UV and UAH are accepted by the user.

## 5. Decisions the user made

| Topic | Decision |
|---|---|
| Platform | Android only for now. |
| Content | The same as today's Dashboard and Calendar, with all current functionality except email-related ones. |
| Screen sizes | Tablet and phone-sized screens. |
| Phone navigation | A **bottom tab bar** with one tap to each section (mockup option A). |
| Tablet navigation | Today's sidebar stays, as on the desktop. |
| Data | Not shared with the desktop. |
| Distribution | The user installs the app file themselves; Google Play is for the future. |
| Test device | The user's Android tablet (real device). An emulator may be used during development. |
| Codebase | The same repository, reusing what exists, per the discussion of the options. |
| Keeping sessions alive | Signing in again when a session expires is acceptable, but the app should keep credentials and sessions going as long as it can. The token or session cookies are kept in secure storage across restarts, and the sign-in `WebView` keeps the login page's remembered session, so a quiet renewal can succeed where the platform allows it. Passwords are still never stored. |

Phone mockup reference (option A). A fixed bar along the bottom has three equal items: Dashboard, Calendar, Universities, each with an icon and a label; the current one is highlighted. Above it, the page title. On Dashboard, the tabs (All, UV, UAH) and then the controls (Sync, Add task, Time/Status) wrap onto their own lines above the sections. Calendar has the ‹ month › header, the seven-column month grid, then the selected day's heading and rows.

## 6. Constraints

- Follow AGENTS.md: planner and sub-agent split, scope rules, security rules, public-repo rule.
- Existing architecture stays: `crates/core` is pure, `crates/infra` holds adapters, `src-tauri` is the composition root, and frontend command calls go only through `src/lib/api/`.
- Technology is Tauri 2's Android support, building on the existing Rust and React code.
- Only necessary dependencies. Any new one must be justified in the plan.
- UI follows the existing conventions: English sentence-case copy, the `en-GB` date helpers, current Tailwind look.
- Touch: anything that relies on hovering must have a touch equivalent.
- Secrets and personal data are never logged.

## 7. The how: what is decided

### 7.1 Sign-in is proven first (a gate)

Sign-in is the one real unknown. The first milestone is a throwaway test on the real tablet, separate from the product code, and everything else waits for its result. It must show:

1. UV (Moodle): the university login page completes inside the app, and the token from the `moodlemobile://` redirect is captured.
2. UAH (Blackboard): the Microsoft Entra login completes inside the app, and the session cookies (including HttpOnly ones) are read.
3. One real API request to each platform succeeds from Rust using those credentials, including TLS (see 7.4).
4. The credential is written to Android's secure storage and read back after restarting the app.
5. Renewal is tried and measured, without blocking the gate: after closing and reopening the app, can a sign-in complete with no user interaction, using the session the login page remembers? The result decides how silent renewal works on Android.

The test also confirms the items in 7.4, reads the tablet's Android version and CPU architecture, and checks the app's security policy (CSP) and permissions on Android.

If any of checks 1 to 4 fails, stop and bring the options to the user before building further. No fallback, such as pasting a token by hand, is assumed. The test's code is not merged.

### 7.2 What differs on Android

- **Sign-in.** The desktop's `lms_auth` opens a second window (`WebviewWindowBuilder`) and uses `on_navigation` and `cookies_for_url`. Tauri on Android has a single window. Android needs its own implementation of the existing `LmsAuthenticator` port and of the sign-in capture. The likely shape is a small Kotlin Tauri plugin hosting an Android `WebView` activity: URL interception for the Moodle redirect, and `CookieManager` for Blackboard. The test in 7.1 confirms or replaces this.
- **Secure storage.** `KeyringSecretStore` (Windows Credential Manager) gets an Android counterpart implementing the existing `SecretStore` port. Values are encrypted with an AES-GCM key held in the Android Keystore and kept in the app's private storage. It must hold values at least as large as the Blackboard cookie sets the desktop accepts. Rust reaches the Kotlin side through a Tauri plugin.
- **Composition.** Email and AI commands, adapters and screens are excluded from the Android build. The core `Repositories` and runner wiring must not require them there.
- **Navigation.** The frontend reuses the existing `EMAIL_FEATURES_ENABLED` build switch (`src/config/features.ts`), which is already off by default since v0.2.0, so sections, routes and the sidebar show only Dashboard, Calendar and Universities. Android builds must never enable it.
- **Layout.** Below a phone-width breakpoint the sidebar is replaced by the bottom tab bar, and pages reflow to one column as in the mockup. On tablet widths the sidebar stays.

### 7.3 What stays shared

`crates/core`, the Moodle and Blackboard clients, the SQLite database and migrations (the email tables simply stay empty on Android), the LMS and personal task commands and DTOs, the React feature code for tracker, calendar, universities and sync, and the API layer.

### 7.4 Known technical risks for the plan

- `reqwest`'s `rustls` feature may use the platform certificate verifier, which needs extra initialisation on Android. Confirm in 7.1.
- `keyring` has no Android backend, so the Android build must not depend on it for secrets.
- `async-imap` and the other email-only dependencies should not be built for Android.
- `tauri-plugin-opener` opens platform links; confirm it on Android.

## 8. Left to the builder

- How email and AI code is excluded (Cargo features or `cfg(mobile)`), provided desktop behaviour is unchanged. Changing build files such as `crates/infra/Cargo.toml` to do it is acceptable.
- The Kotlin plugin's structure and names.
- The bottom tab bar's icons and exact styles, and the exact breakpoint.
- How the frontend knows it is on Android.

## 9. Calls I made (unconfirmed by the user)

- **Universities stays**, although not named by the user, because the dashboard has no data without it.
- **Settings is removed** on Android, since it holds only email and AI settings.
- **Dot tooltips on touch:** tapping a day already lists its entries by title, so the day list is the touch equivalent of hovering a dot. No extra popup is added.
- **Silent sign-in renewal:** the app tries a quiet renewal through the `WebView`'s remembered login session. If that cannot work or fails, the existing "sign in again" banner handles expired sessions.
- **Offline:** already-synced items and personal tasks remain usable without internet; Sync shows its normal error.
- **Install file:** a signed release APK the user can install, built locally. The signing key stays out of the repository.
- **Android version floor:** whatever Tauri's Android template sets by default, unless the tablet needs lower.
- **How the work is planned:** the plan details the sign-in test and all work that doesn't depend on its result. The product sign-in task is specified after the test, using what it showed.
- **Git:** one phase branch, `phase-4-android`, from `main`, with feature branches into it. The test lives on a throwaway branch. The generated `gen/android` project is committed only after the test passes.
- **Sign-in screen:** a separate Android screen hosting a `WebView`, because a Custom Tab can't read HttpOnly cookies. Back or cancel returns to the app with sign-in cancelled, as closing the window does on the desktop.
- **Signing out:** "Sign out" also clears the sign-in `WebView`'s cookies for that university's platform, so the next sign-in starts clean.
- **Signing key:** a release key generated on this PC and kept outside the repository, referenced from a git-ignored file. A debug-signed APK is fine until the first real delivery.
- **App lifecycle:** if Android kills the app during a sync, the sync is simply lost and the user presses Sync again. The database stays in the app's private data folder.
- **Toolchain:** installing the Android SDK, NDK and Java is handled separately and is not part of the plan.

## 10. Testing

- Existing Rust and frontend suites stay green, and the desktop build still passes.
- New unit tests for the Android-independent logic added (navigation filtering, layout switching at the breakpoint).
- Kotlin and sign-in behaviour is verified on the real tablet with a manual acceptance checklist in `docs/testing.md`, covering sign-in to both universities, sync, Dashboard, Calendar, personal tasks, restart persistence, phone-width layout (emulator), and the absence of any email or AI section.

## 11. As built

What was shipped, and where it differs from the sections above. Values are invented or public.

**Gate (Xiaomi Pad 7, Android 16 / API 36, arm64-v8a).** Moodle (UV): the `WebView` captured the `moodlemobile://` redirect, the token decoded and an authenticated request worked. Blackboard (UAH): the Microsoft login completed in the `WebView`, `CookieManager` returned the session cookies (about 1.1 KB, under the store's limit) and an authenticated request worked. The Keystore store survived a force-stop and relaunch, and quiet renewal captured a fresh credential silently for both. Backing out of the sign-in screen returned "cancelled" (verified on the emulator).

**Deviations and rulings.**

- **R2, TLS roots:** on Android `http_client()` uses bundled `webpki-roots` through `reqwest::ClientBuilder::use_preconfigured_tls`, as the IMAP code already does, because the default platform verifier panics without JNI initialisation. Cost: new CA roots need an app update and user-installed CAs are not honoured.
- **R3, SDK level:** `compileSdk` and `targetSdk` are 36 (installed) instead of the template's 37.
- **R4, Windows build:** `tauri android build` fails on Windows without Developer Mode because it creates a symbolic link; the manual steps in `docs/development.md` build the same APK.
- **R6, leaving the app:** the root screen's Back button calls the Android-only command `android_leave_app` (Kotlin `moveTaskToBack`), because closing the Tauri window does not leave the app.
- **R7, insets:** `MainActivity` applies the system-bar and cutout insets as padding, because CSS `env(safe-area-inset-*)` is 0 in this `WebView`. `MainActivity.kt` is generated, so re-apply the change after a new `tauri android init`.
- **R9 / R12, sign-out:** signing out clears the sign-in `WebView`'s cookies (shared by both universities) and the storage of the origins the sign-in screen visited (recorded as scheme, host and port only), never the app's own origin. The other university's next quiet renewal may therefore need one interactive sign-in.
- **R11, quiet renewal:** the invisible sign-in screen is translucent, not touchable and not focusable, so it never covers the app while it runs (up to 20 seconds).
- **Identifier:** `tauri android init` turns the hyphen into an underscore: `dev.eaybar.academic_assistant`.
- **Release signing:** the key is generated with `keytool` outside the repository; `src-tauri/gen/android/keystore.properties` (git-ignored) points to it. Without that file the release build is unsigned and says so.

**Not verified:** the Microsoft login's origins were not recorded on the emulator (the same mechanism was verified with another single sign-on host); IME insets; a physical phone.
