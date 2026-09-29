# Academic Assistant

Academic Assistant is a Windows desktop application that watches your university email
accounts, filters out the noise, and turns the relevant messages into structured academic
information: category, priority, course, university and dates, plus flags for "action
required" and "possible change". You press **Check now**, and the app shows only what you
have not reviewed yet.

It is built with Tauri 2 (Rust) and React. Analysis is done by an interchangeable
structured AI provider: **jev** (by TypeSafe, external, needs an API key) or **Ollama**
(local, nothing leaves your PC). The choice is explicit and lives in Settings.

## Status: Phase 1 (email analysis)

### In scope

- Multiple email accounts managed from the UI: IMAP (username + password) and
  Microsoft 365 through Microsoft Graph (OAuth 2, PKCE, no client secret).
- Manual **Check now** across all enabled accounts, with per-account error isolation.
- Incremental synchronization; the first sync covers a configurable number of past days
  (default 14) per account.
- Deduplication and conservative deterministic filters (blocked senders/domains, own
  messages, empty messages) before any AI call.
- Subject and body analysis only, with typed questions and closed answer sets. The AI never
  computes dates; it classifies date candidates found by deterministic code.
- Manual catalog of universities and courses (with aliases) that the AI uses to identify the
  course.
- Results history with new/seen status (including undoing "seen"), search, filters and
  sorting; run history with per-account detail and errors.
- Run log and application log. The full email body is never stored.

### Out of scope (later phases)

Scheduled/background checks, Windows notifications, linking related messages and computing
changes between them (Phase 1 only flags a "possible change"), free-text summaries, other
sources (Moodle, Blackboard, calendars, course-page scraping), attachments and the final
visual design.

## Quick start

Prerequisites: Windows 11, Rust stable (MSVC) with Visual Studio Build Tools (C++
workload), Node 22+ and npm. Full details, including the Microsoft Entra registration and
the optional Ollama/jev setup, are in [docs/setup.md](docs/setup.md).

```powershell
npm install
npm run tauri dev
```

Then, in the running app:

1. **Settings**: choose the AI provider (Ollama runs locally after `ollama pull qwen3:8b`;
   jev needs an API key entered in Settings) and, for Microsoft 365, paste the Entra
   Application (client) ID.
2. **Accounts**: add an account and press Test.
3. **Dashboard**: press **Check now**.

To produce an installer: `npm run tauri build` (see [docs/setup.md](docs/setup.md)).

## Documentation

Start with the [documentation index](docs/README.md). The most useful entry points:

| Document | Purpose |
|---|---|
| [docs/setup.md](docs/setup.md) | Prerequisites, Microsoft Entra registration, running and building |
| [docs/user-guide.md](docs/user-guide.md) | Every screen, how a check works, privacy, troubleshooting |
| [docs/architecture.md](docs/architecture.md) | Crates, data flow, data model, security model, extension points |
| [docs/development.md](docs/development.md) | Repo layout, conventions, quality gates, how to add things |
| [docs/testing.md](docs/testing.md) | Test suites, live tests, manual acceptance checklist |
| [requirements.md](requirements.md), [technical-specification.md](technical-specification.md) | Functional requirements and overall technical direction |

## Privacy at a glance

- Passwords, refresh tokens and the jev API key are stored only in Windows Credential
  Manager, never in the database, logs or files.
- Mailboxes are accessed read-only; messages are never marked as read, moved or deleted.
- Email bodies exist only in memory during processing. The database keeps structured
  results and the metadata needed for deduplication.
- With Ollama, nothing leaves the computer except the mail traffic itself. With jev, the
  subject, sender and normalized body of messages that pass the filters are sent to
  TypeSafe.
