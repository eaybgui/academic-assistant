# Academic Assistant — Functional requirements

## 1. Goal
Academic Assistant is a desktop application that centralizes and automates tracking academic information from several universities. Its goal is to detect relevant news in email, classify it and structure it using AI, so the user does not have to check every source manually.

The first version focuses on email. The architecture must allow adding Moodle, Blackboard, calendars, external web pages (scraping course information) and other sources later.

## 2. Phases

### Phase 1 — Email analysis (current)
Includes:
- Management of multiple email accounts from the UI (IMAP and Microsoft 365).
- Manual checks ("Check now") across all enabled accounts.
- Incremental synchronization; the first sync covers a configurable number of past days.
- Deterministic filters before AI.
- Analysis of subject and body only.
- AI classification and structured extraction (relevance, category, priority, action required, possible change, university, course, dates).
- Manual catalog of universities and courses used by the AI to identify the course.
- Persistence of AI results and links to the original emails.
- New/seen status (including undoing "seen").
- Dashboard, results history (search, filters, sorting) and run history.
- Run and error logging.
- Fully local operation (local AI provider) or an explicitly configured external AI provider (API key).

Excluded from Phase 1 (planned for later phases):
- Automatic scheduled checks and background execution.
- Native Windows notifications.
- Linking related communications and computing changes between them (Phase 1 only flags "possible change").
- Free-text summaries.

### Later phases (not scheduled)
- Scraping external web pages to gather course information (feeds the course catalog).
- Moodle (UV), Blackboard (UAH), calendars.
- Attachments.
- Browser agents.
- Notion.
- Final visual design of all screens.
- LLM comparison.

## 3. Account management
The user can add, edit, enable/disable, delete and test accounts from the application. It is not limited to UV and UAH.

The architecture must support different access methods. Phase 1 implements IMAP (username + password) and Microsoft 365 through Microsoft Graph (OAuth2 sign-in). Credentials and tokens are never stored as plain text in SQLite.

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

## 7. Persistence
The application keeps structured AI results, the metadata needed for deduplication, synchronization state and the run log. The full email body is never stored; it only exists in memory during processing.

Results must allow consulting what happened, when it was detected, which account it came from, its classification and the link to the original message.

## 8. New/seen status
Unreviewed results are distinguished from seen ones. The user can mark results as seen (individually or all at once), mark them as new again, and browse the history.

## 9. User interface
Navigation: Dashboard, Results, Runs, Accounts, Catalog, Settings.

- **Dashboard:** last check (time and status), "Check now" with progress, a warning when an account needs attention, counters of unseen results (new, tasks, exams, events, changes and notices) and the list of new results with sorting options (nearest upcoming date — default —, priority, received date).
- **Results:** full history with text search (subject/sender) and a "Filters and sort" modal (state, category, account, course, sort order). Rows expand to show details.
- **Runs:** run history with per-account detail and errors.
- **Accounts:** list and add/edit form (IMAP or Microsoft 365), test connection, enable/disable, reconnect, delete.
- **Catalog:** universities and their courses (with aliases/codes).
- **Settings:** AI provider selection, provider configuration, Microsoft sign-in configuration and blocked senders/domains.

The UI language is English. The visual design of Phase 1 is functional, not final.

## 10. Automation
- Phase 1: manual execution.
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
Passwords, tokens and API keys are never stored as plain text. No information is sent to external services unless the user configures an external provider. Persisted information is minimized.

## 15. Extensibility
The architecture must allow adding later:
- UV Moodle;
- UAH Blackboard;
- calendars;
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
