# Academic Assistant

Academic Assistant is a Windows desktop application that keeps track of your university
deliverables, exams and course dates in one place. You connect each university's learning
platform (Moodle or Blackboard), choose the courses to follow, and the dashboard shows
assignments, quizzes and course events from every university, grouped by time or by status, with
badges for what is new or has a changed date. Email analysis is kept as a secondary
**Announcements** section: it watches your university mailboxes and turns the relevant messages
into structured information.

It is built with Tauri 2 (Rust) and React. The learning-platform tracker uses no AI. Email
analysis is done by an interchangeable structured AI provider: **jev** (by TypeSafe, external,
needs an API key) or **Ollama** (local, nothing leaves your PC). The choice is explicit and lives
in Settings.

## Status: Phase 2 (LMS tracker)

### In scope

- **Universities**: add a Moodle or Blackboard Learn platform by name and URL (the URL is checked
  against the platform), sign in through the university's own single sign-on in an embedded
  window, and see "Connected as ...". Sessions are checked and renewed silently when possible.
- **Course selection**: courses are discovered from the platform (all pre-selected) and can be
  edited later; courses that appear later are surfaced for review.
- **Sync** (manual, all universities or one): assignments, quizzes, tests and course calendar
  events, with submission status and grades. Changes since the last sync are shown as **New** and
  **Date changed** badges until dismissed; items that disappear from the platform are kept and
  flagged.
- **Dashboard**: an "All" tab plus one per university, grouped by time (Overdue, Today, This week,
  Next week, Later, No date) or by status; mark items done, hide them, add notes, open them in the
  platform. Personal state is stored locally and never overwritten by a sync.
- **Announcements** (email analysis from Phase 1): multiple IMAP and Microsoft 365 accounts,
  manual **Check now**, deterministic filters, AI classification of subject and body only, results
  history with new/seen status, run history and logs. It stays disabled until at least one course
  is tracked, because the tracked courses are its course catalog.
- Run log and application log. Full email bodies and your university password are never stored.

### Out of scope (later phases)

Google Calendar export, scheduled/background sync and Windows notifications, platform
announcements, Moodle activity types other than assignments and quizzes, a month calendar view,
linking related emails and computing changes between them, attachments and the final visual
design.

## Quick start

Prerequisites: Windows 11, Rust stable (MSVC) with Visual Studio Build Tools (C++
workload), Node 22+ and npm. Full details, including the Microsoft Entra registration and
the optional Ollama/jev setup, are in [docs/setup.md](docs/setup.md).

```powershell
npm install
npm run tauri dev
```

Then, in the running app:

1. **Universities**: press **Add university**, enter a name, the platform and its URL, sign in in
   the window that opens, choose your courses and press **Save and sync**.
2. **Dashboard**: review your deliverables; press **Sync** to refresh.
3. Optional, for email: **Settings** (choose the AI provider: Ollama runs locally after
   `ollama pull qwen3:8b`; jev needs an API key; for Microsoft 365 paste the Entra Application
   (client) ID), then **Email accounts** (add an account and press Test), then
   **Announcements** and **Check now**.

To produce an installer: `npm run tauri build` (see [docs/setup.md](docs/setup.md)).

## Documentation

Start with the [documentation index](docs/README.md). The most useful entry points:

| Document | Purpose |
|---|---|
| [docs/setup.md](docs/setup.md) | Prerequisites, Microsoft Entra registration, running and building |
| [docs/user-guide.md](docs/user-guide.md) | Every screen, how a sync and an email check work, privacy, troubleshooting |
| [docs/architecture.md](docs/architecture.md) | Crates, data flow, data model, security model, extension points |
| [docs/development.md](docs/development.md) | Repo layout, conventions, quality gates, how to add things |
| [docs/testing.md](docs/testing.md) | Test suites, live tests, capture tool, manual acceptance checklists (including Phase 2) |
| [requirements.md](requirements.md), [technical-specification.md](technical-specification.md) | Functional requirements and overall technical direction |

## Privacy at a glance

- Your university password is never captured or stored. Signing in to a learning platform stores
  only a scoped credential (a Moodle mobile web-service token, or the Blackboard session cookies
  of the platform host).
- That credential, email passwords, Microsoft refresh tokens and the jev API key are stored only
  in Windows Credential Manager, never in the database, logs or files.
- Learning platforms are accessed read-only, and the app talks only to the platforms you add.
  Nothing from them is sent anywhere else, and no AI is involved in the tracker.
- Mailboxes are accessed read-only; messages are never marked as read, moved or deleted.
- Email bodies exist only in memory during processing. The database keeps structured
  results and the metadata needed for deduplication.
- The only data that can leave the computer for AI is, when jev is selected, the subject, sender
  and normalized body of messages that pass the filters, sent to TypeSafe. With Ollama, nothing
  leaves the computer except the mail traffic itself.
- Item titles, notes and grades are personal data: they are kept in the local database only and
  are never written to the logs.
