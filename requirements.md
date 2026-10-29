# Academic Assistant — Functional requirements

## 1. Goal
Academic Assistant is a desktop application that centralizes and automates tracking academic information from several universities. Its goal is to show the user, in one place, the deliverables, exams and important dates of every university, so they do not have to check every platform and mailbox manually.

Since Phase 2 the source of truth is the universities' learning platforms (Moodle and Blackboard), read through their APIs without AI. Email analysis (Phase 1) is kept as a secondary "Announcements" section. The architecture must allow adding calendars, external web pages (scraping course information) and other sources later.

## 2. Phases

### Phase 1 — Email analysis (done; now the "Announcements" section)
Includes:
- Management of multiple email accounts from the UI (IMAP and Microsoft 365).
- Manual checks ("Check now") across all enabled accounts.
- Incremental synchronization; the first sync covers a configurable number of past days.
- Deterministic filters before AI.
- Analysis of subject and body only.
- AI classification and structured extraction (relevance, category, priority, action required, possible change, university, course, dates).
- A catalog of universities and courses used by the AI to identify the course (in Phase 1 entered manually; since Phase 2 it is the tracked courses of the LMS universities).
- Persistence of AI results and links to the original emails.
- New/seen status (including undoing "seen").
- Results history (search, filters, sorting), unseen counters and run history (originally a Dashboard; now shown in Announcements and Email runs).
- Run and error logging.
- Fully local operation (local AI provider) or an explicitly configured external AI provider (API key).

Excluded from Phase 1 (planned for later phases):
- Automatic scheduled checks and background execution.
- Native Windows notifications.
- Linking related communications and computing changes between them (Phase 1 only flags "possible change").
- Free-text summaries.

### Phase 2 — LMS tracker (current)
The product is re-centred on tracking deliverables, exams and important dates. No AI is involved.

Includes:
- Universities configured by the user: name, platform (`moodle` or `blackboard`) and platform URL, validated against a public endpoint of the platform.
- Sign-in through the university's own single sign-on in an embedded window. Only a scoped credential is kept (a Moodle mobile web-service token, or the Blackboard session cookies of the platform host); the university password is never captured or stored. Every sign-in is verified; expired sessions are renewed silently when possible, otherwise the user is asked to sign in again.
- Course discovery from the platform and course selection (all pre-selected, editable later; courses found later are added untracked and flagged for review).
- Manual sync (all universities or one) of assignments, quizzes and tests (with submission status and grade) and course calendar events.
- Change tracking since the last sync: "new" and "date changed" badges until dismissed; items that disappear from the platform are kept and flagged.
- Personal per-item state stored locally and never overwritten by a sync: done, hidden, note.
- Dashboard: an "All" tab and one tab per university, grouped by time or by status, with a details view that opens the item in the platform.
- Announcements: the Phase 1 email analysis, disabled until at least one course is tracked.

Excluded from Phase 2 (planned for later phases):
- Google Calendar export (Google Calendar API, dedicated calendar; item ids are stable to allow it).
- Automatic background sync and native Windows notifications.
- Platform announcements feeding the Announcements section.
- Moodle activity types other than assignments and quizzes; a month calendar view.

### Later phases (not scheduled)
- Scraping external web pages to gather course information.
- Calendars.
- Attachments.
- Browser agents.
- Notion.
- Final visual design of all screens.
- LLM comparison.

## 3. Account management
The user can add, edit, enable/disable, delete and test accounts from the application. It is not limited to UV and UAH.

The architecture must support different access methods. Email accounts support IMAP (username + password) and Microsoft 365 through Microsoft Graph (OAuth2 sign-in). Credentials and tokens are never stored as plain text in SQLite.

Universities (learning platforms) are managed separately from email accounts: see section 9 and the Phase 2 design.

Each account can have a default university and a configurable first-sync window (default 14 days).

## 4. Fetching and filtering
Each run finds messages that are new since the last synchronization. Before AI, deterministic filters are applied based on, among others, sender, domain, own messages, folder, identifiers, processing state and duplicates.

Filters only discard clearly irrelevant information; they do not replace the AI's semantic classification. The default strategy is conservative to avoid false negatives.

Email accounts are accessed read-only: the application never marks messages as read, moves or deletes them.

## 5. AI analysis
Phase 1 analyzes subject and body only. For each selected communication it produces structured information:

- relevance;
- university;
- course;
- professor/sender;
- category;
- action required;
- priority;
- dates and deadlines, each classified by type (exam, deadline, event, class, other);
- possible change with respect to previously known information;
- link to the original email (or its Message-ID when the protocol provides no link).

Minimum categories: notice, task, exam, event, change and general information.

The AI providers used produce only structured answers (typed questions with closed answer sets). Free-text summaries are out of scope for Phase 1.

## 6. Change detection
The application must identify modifications of previously known information, such as changes of exam or submission dates, rooms, cancellations or new instructions.

Phase 1 flags communications that announce a possible change. Later phases will relate communications to each other instead of treating every email as an independent item.

For learning-platform items (Phase 2) changes are detected deterministically by comparing each sync with the stored data: an item that is new since the previous sync of its course, an item whose due date changed (the original date is remembered, and returning to it clears the badge), and an item the platform no longer returns (kept and flagged as removed).

## 7. Persistence
The application keeps structured AI results, the metadata needed for deduplication, synchronization state and the run log. The full email body is never stored; it only exists in memory during processing.

Results must allow consulting what happened, when it was detected, which account it came from, its classification and the link to the original message.

## 8. New/seen status
Unreviewed results are distinguished from seen ones. The user can mark results as seen (individually or all at once), mark them as new again, and browse the history.

## 9. User interface
Navigation: Dashboard, Announcements, Universities, Email accounts, Email runs, Settings.

- **Dashboard (`/`):** the LMS tracker. An "All" tab and one tab per university; "Sync" for the selected university (or all); the last sync time; grouping by time (Overdue, Today, This week, Next week, Later, No date, with finished items in a collapsed section) or by status (Pending, Submitted, Graded, past events); "Show hidden" and "Show removed" filters. Each item shows its university, title, course, kind, status, grade, due date, change badge (with dismiss), a done checkbox and a note indicator. Clicking an item opens its details (full dates, grade, note editor, hide, mark done, "Open in platform"). Banners ask the user to sign in again when a session expired and summarize each sync (new, date changed, removed, failures per university). With no university the dashboard offers to add one.
- **Universities:** one card per university with platform, URL, "Connected as ...", login state, last sync, tracked courses and a "new courses found" link; Sign in, Sign out, Check connection, Sync, Edit courses, Rename and Delete. An add wizard: details, sign in, choose courses and sync.
- **Announcements:** the email results. A header with "Check now" and its progress, the last check, attention warnings for accounts that need action and counters of unseen results (new, tasks, exams, events, changes and notices); below it the full history with text search (subject/sender), a "Filters and sort" modal (state, category, account, course, sort order) and expandable rows. Disabled while no course is tracked.
- **Email accounts:** list and add/edit form (IMAP or Microsoft 365), test connection, enable/disable, reconnect, delete.
- **Email runs:** run history with per-account detail and errors.
- **Settings:** AI provider selection, provider configuration, Microsoft sign-in configuration and blocked senders/domains.

The UI language is English. The visual design is functional, not final.

## 10. Automation
- Phases 1 and 2: manual execution (Check now for email, Sync for the learning platforms).
- Later phase: 1–2 automatic executions per day, configurable times, execution even when the main window is not open.

Each run has a status and statistics. A failure in one account does not prevent processing the others.

## 11. Notifications (later phase)
Native Windows notifications to report, when appropriate, completion, relevant news and important errors.

## 12. Logging
At minimum, the following are recorded:
- start/end;
- processed account;
- messages found;
- messages discarded;
- messages sent to AI;
- results created;
- connection/authentication/AI errors.

## 13. Local AI and external API
The application must be able to work without sending academic information outside the computer.

Two options exist:
1. local AI provider (Ollama);
2. external AI provider configured with an API key (jev, by TypeSafe).

The provider is interchangeable and the choice is explicit.

## 14. Privacy
Passwords, tokens, session cookies and API keys are never stored as plain text; they live only in Windows Credential Manager. The university password is never captured. Learning platforms are accessed read-only, and nothing obtained from them is sent anywhere except, for email analysis, the AI provider the user chose. No information is sent to external services unless the user configures an external provider. Persisted information is minimized, and item titles, notes and grades are never written to logs.

## 15. Extensibility
The architecture must allow adding later (Moodle and Blackboard are implemented since Phase 2):
- Google Calendar export;
- other calendars;
- web pages (course information scraping);
- other platforms.

## 16. Functional principles
1. Automate collection.
2. Detect changes before using AI when possible.
3. Minimize unnecessary processing.
4. Turn communications into structured information.
5. Keep a useful history without storing full emails.
6. Always show the status of the last check.
7. Decouple sources and AI.
8. Prioritize privacy and local operation.
9. Allow adding accounts and sources.
10. The database is the source of truth, not the UI.
