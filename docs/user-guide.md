# User guide

This guide describes what you can do in Academic Assistant and what happens to your data. If
you have not installed or configured anything yet, start with [setup.md](setup.md).

Academic Assistant has two parts. The **Dashboard** tracks your deliverables, exams and course
dates from your universities' learning platforms (Moodle and Blackboard). **Announcements**
analyzes university email with an AI provider and is optional.

## First-time checklist

1. **Universities**: press **Add university**, enter a name, choose the platform and enter its web
   address, sign in with your university account in the window that opens, choose your courses
   and press **Save and sync**.
2. **Dashboard**: check your deliverables. Press **Sync** whenever you want fresh data.
3. Optional, for email: **Settings** (pick the AI provider: with **Ollama**, make sure Ollama is
   running and the model is pulled (`ollama pull qwen3:8b`); with **jev**, use Replace key to
   enter your API key; for Microsoft 365, enter the Entra application (client) ID), then
   **Email accounts** (add each mailbox and press **Test**), then **Announcements** and **Check
   now**.

## The screens

The left sidebar (collapse it to icons with the arrow button at its top; the app remembers your
choice) links to Dashboard, Announcements (with a badge showing the number of unseen results),
Universities, Email accounts, Email runs and Settings. Announcements is greyed out, with the hint
"Add a university and track courses first", until at least one course is tracked.

### Dashboard

Shows the deliverables, exams and course events of the courses you track.

- **Tabs**: **All** plus one tab per university. The app remembers the tab you used last.
- **Sync**: refreshes the selected university (or all of them on the All tab). The button shows
  the progress, for example `Universidad Demo: Fetching items`. Next to it, "Last synced" shows
  when the data was last refreshed. You can navigate elsewhere while a sync runs.
- **Group by**: **Time** (Overdue, Today, This week, Next week, Later, No date; weeks start on
  Monday) or **Status** (Pending, with overdue items first, Submitted and Graded). Finished items
  are collected in a collapsed **Completed** section (**Past events** when grouped by status).
  Inside each group items are sorted by due date, undated ones last. The app remembers your
  choice.
- **Show hidden** and **Show removed**: both off by default. Hidden items are ones you hid;
  removed items are ones the platform no longer returns.
- **Item row**: the university (on the All tab), the title, the course and kind (Assignment,
  Quiz, Test, Event or Other), a status pill, the grade when there is one, the due date and a
  checkbox to mark it done (not shown for events). A pencil mark means the item has a note.
- **Status**: **Pending**, **Overdue** (past due and not handed in), **Submitted** (the platform
  says so, or you marked it done), **Graded**, or **Past** (an event that has happened, or
  something with nothing to submit whose date has passed).
- **Badges**: **New** marks an item that appeared since the last sync of its course (nothing is
  badged during a course's first sync); **Date changed: 5 Oct -> 12 Oct** shows the previous and
  the new date. Press the x to dismiss one badge, or **Dismiss all changes** in the summary after
  a sync.
- **Details**: click a title to see the full dates, the grade, "Removed from the platform on ..."
  when applicable, **Open in platform** (opens the item in your browser), a **Note** (up to 2000
  characters; saving an empty note clears it), **Hide** / **Show again** and **Mark as done**.
  Your done, hidden and note settings live only on your PC and are never changed by a sync.
- **Banners**: "<university> needs you to sign in again" with a **Sign in** button when a session
  expired (after signing in, that university syncs again); after each sync a summary such as
  "Sync finished: 3 new, 1 date changed, 1 removed", with any university that failed listed with
  its error.
- With no university the Dashboard offers **Add your first university**; with universities but no
  tracked course it offers **Choose courses**.

### Universities

One card per university: its platform and address, the connection ("Connected as Ana Pérez",
"Not signed in", "Session expired" or "Sign-in failed"), "3 of 12 courses tracked", the time of the
last sync and, if it failed, the error.

- **Add university**: a three-step window. (1) Name, platform and web address; the app checks that
  the address really is a Moodle or Blackboard site and refuses a platform you already added.
  (2) **Sign in**: your university's own sign-in page opens in a separate window; when you finish,
  the window closes by itself and the card shows "Connected as ...". (3) Choose your courses (all
  are ticked) and press **Save and sync**. If you close the window after step 1, the university
  stays saved as "Not signed in" and you can continue from its card.
- **Sync**: refresh just this university.
- **Edit courses**: the course list is refreshed from the platform (if that is not possible, for
  example because your session expired, the last known list is shown together with the error).
  Courses new to the app carry a **New** tag; courses the platform no longer shows say "Not visible
  on the platform". Saving starts a sync of the university. A link "N new courses found" on the
  card opens the same window.
- **Check connection**: verifies the saved sign-in and renews it quietly if it expired; if that is
  not possible the card shows "Session expired" and you press **Sign in**.
- **Sign in / Sign out**: signing out deletes the saved credential. Your courses and items stay
  until you delete the university.
- **Rename**: changes the display name only. The platform and its address cannot be changed;
  delete the university and add it again.
- **Delete**: removes the university, its courses, its items (including your notes) and the saved
  credential.

### Announcements

The analyzed emails, with a header: the last check and its status, **Check now**, warnings for
email accounts that need attention, counters of unseen results (New, Tasks, Exams, Events, and
Changes and notices) and **Mark all as seen**.

- **Check now**: starts a manual check of all enabled email accounts. While it runs, the button is
  replaced by a progress label such as `UV IMAP: Analyzing 12/40`. You can navigate to other
  screens meanwhile; the run continues. It is refused with "Track at least one course before
  checking email" until a course is tracked on the Universities page, because your tracked courses
  are what the AI uses to name the course of each email.
- **Attention banners**: one per enabled account that needs action, for example an account whose
  last sync failed, or one that has no stored password or Microsoft sign-in.
- **Results**: the full history. Use the search box (subject, sender name and address) and the
  **Filters and sort** button, which opens a window with state (new/seen), category, account,
  course and sort order; a badge shows how many filters are active. Apply commits your choices,
  Reset clears them. Rows show subject, category, next date and priority, with a dot for unseen
  results. Click a subject to expand the row: sender with address, account, received and detected
  dates, all dates, the tags **Action required** and **Possible change**, **Open email**
  (Microsoft 365 results, opens the message in your browser) or **Copy Message-ID** (IMAP results,
  which have no link), and **Mark as seen** / **Mark as new** (undo). Results are paged 25 per
  page.

### Email runs

History of the latest 50 email checks, newest first: status (Running, Success, Partial, Failed), start time,
duration, trigger (Manual) and counters (found, filtered, analyzed, failed, results). Expand a
run to see its error summary and each account's status; a failed account shows an error kind
(authentication, connection, protocol, AI, internal) and a message.

### Email accounts

One row per account: provider, email, default university, folders and last sync status.

- **Add account**: switch between **IMAP** and **Microsoft 365**.
  - IMAP fields: display name, email, host, port (default 993, implicit TLS), username,
    password, folders (default `INBOX`), default university, first-sync days.
  - Microsoft 365 fields: display name, email, folders (default `inbox`), default university,
    first-sync days. After saving, press **Sign in with Microsoft**; your browser opens, you
    sign in, and the app receives the result on a local loopback address (5 minute limit). The
    email address entered must match the mailbox you sign in with (its `mail` address or its
    user principal name), otherwise nothing is stored.
  - If you change the email address of a Microsoft 365 account, its sign-in is discarded and
    you have to sign in again (the tenant cannot be changed after creation).
  - The provider type cannot be changed after creation. When editing an IMAP account, leave
    the password empty to keep the stored one.
  - Secrets are saved in Windows Credential Manager, never in the database.
- **Test**: checks the connection and shows the result inline.
- **Edit**, **Enabled** toggle (disabled accounts are skipped by checks).
- **Reconnect** / **Sign in** (Microsoft 365 only): repeat the sign-in, for example when it
  expired.
- **Delete**: removes the account, its stored secrets and **all its results**. Disable the
  account instead to keep its history.

### Settings

- **AI provider**: **Ollama** ("Stays on this PC") or **jev** ("Sends email text to
  TypeSafe"). The default is Ollama.
- **jev**: API key (write-only: it shows only "Configured" or "Not configured", with Replace
  key and Remove key), and the model name (default `jev-latest`).
- **Ollama**: base URL (default `http://localhost:11434`) and model (default `qwen3:8b`).
- **Test provider**: asks the selected provider one fixed question and reports the result.
- **Entra application (client) ID**: needed for Microsoft 365 accounts ([setup.md](setup.md)).
- **Blocked senders and domains**: one per line. An entry starting with `@` (for example
  `@newsletter.example.com`) blocks that exact domain (not its subdomains); any other entry
  blocks that exact address. Matching is case-insensitive. Blocked messages are discarded
  before the AI sees them.

Press Save to apply the whole form.

## How a sync works

1. Press **Sync** (or save your courses, which starts one). The app only syncs universities that
   are signed in, one after another; if another sync is already running you are told so.
2. For each university it first checks that your saved sign-in still works, renewing it quietly
   when it can. If it cannot, that university is marked "needs you to sign in again" and the rest
   continue.
3. It refreshes the course list (new courses appear untracked and are offered for review), then
   fetches the assignments, quizzes, tests and course events of the courses you track. Course
   events are read from 180 days back to one year ahead.
4. It compares what it found with what it stored: new items, items whose due date changed, and
   items the platform no longer returns (kept and shown as "Removed from platform"). Your done,
   hidden and note settings are never touched. If a university fails midway, nothing from it is
   saved and the previous data stays.
5. A summary appears on the Dashboard. Running a second sync right away shows no badges unless
   something changed on the platform.

## How an email check works

1. The app verifies it can start: no other run in progress, at least one enabled account, and
   a usable AI provider (jev selected without a key is refused with a message).
2. For each enabled account, in order, and for each configured folder, it fetches the
   messages that are new since the last check:
   - **First sync** (no stored position yet): messages from the last *first-sync days* of the
     account (default 14).
   - **Later checks**: only messages newer than the stored position. If the provider's
     mailbox was recreated, the app starts over safely; already-processed messages are
     recognized and skipped.
   - At most 200 messages are fetched per folder per check; the next check continues from
     there.
3. Each message goes through deduplication, the filters (blocked sender or domain, your own
   messages, empty messages), date extraction and, if it survives, one AI request that
   classifies it. Irrelevant messages are recorded but produce no result.
4. Relevant messages become results in Announcements.
5. The run ends **Success**, **Partial** (some accounts failed) or **Failed** (all failed).
   A failure in one account never stops the others.

If the AI provider is unreachable or rejects the key, that account stops and its position is
**not** advanced, so the same messages are analyzed on the next check. A single message the AI
answers with something unusable is marked failed and skipped; the rest continue.

### What the tags mean

- **Category**: notice, task, exam, event, change or general information.
- **Priority**: low, medium, high or urgent, based on how soon something must be done.
- **Action required**: the email asks you to do something (submit, register, reply, attend,
  prepare, pay or bring something).
- **Possible change**: the email seems to modify something announced before (a new date or
  time, a new room, a cancellation, new instructions). The app only flags this for email; it
  does not link the message to the earlier one or compute what changed. (The Dashboard, by
  contrast, detects changed dates exactly, by comparing syncs.)
- **Dates**: the app finds date mentions (Spanish and English formats, plus "today",
  "tomorrow", "next Monday") and the AI only classifies what each one refers to. Dates the AI
  judges unrelated are dropped.

## Privacy

- **Stored on your PC** (SQLite): your universities (name, platform, address, sign-in state),
  courses and tracked items (title, due date, status, grade, link, and your own done/hidden/note
  settings), email accounts (without secrets), settings, run history, message identifiers and a
  content hash for deduplication, and for each relevant message its subject, sender, category,
  priority, dates, flags, chosen course and university, and the model used. **Email bodies are
  never stored.**
- **Stored in Windows Credential Manager**: IMAP passwords, Microsoft refresh tokens, the jev API
  key, and for each university a scoped sign-in credential (for Moodle a mobile web-service
  token, for Blackboard the session cookies of the platform). **Your university password is never
  captured or stored**: you type it only on your university's own sign-in page, in a separate
  window that has no access to the app. Signing out or deleting the university removes the
  credential. In Credential Manager the entries are named like
  `academic-assistant/university/1/moodle-token.academic-assistant`.
- **Read-only access**: the app only reads from your learning platforms and mailboxes. It never
  submits, edits or deletes anything on a platform, and never marks messages as read, moves,
  deletes or sends.
- **Where data goes**: the app talks only to the platform addresses you add and, for email, to
  your mail servers. Nothing from the platforms is sent anywhere else, and the tracker uses no AI.
- **Sent to jev** (only if selected): for each message that passed the filters, the subject,
  sender name and address, received time, the cleaned body (quotes and signatures removed, at
  most 12,000 characters), the account's default university name, and the names of your tracked
  courses. With **Ollama**, the same content is sent only to your local Ollama server.
- The application log contains ids, counters and sanitized errors, never message content, item
  titles, notes, grades or secrets, and never the addresses visited in the sign-in window.

## Troubleshooting

| Symptom | What to do |
|---|---|
| "This URL is not a Moodle site", "This URL is not a Blackboard Learn site" | The address does not match the platform you chose. Use the address you open in your browser to reach the platform, and pick the right platform. |
| "Mobile web services are disabled on this Moodle site" | Your university's Moodle does not allow the mobile app service, which the tracker needs. Ask the Moodle administrators to enable it. |
| "This platform is already added" | That address is already on the Universities page. |
| "Sign-in cancelled" or "Sign-in timed out" | The sign-in window was closed, or left open for 10 minutes. Press **Sign in** again. |
| "A sign-in window for this university is already open" | Finish or close the existing sign-in window. |
| "<university> needs you to sign in again" or "Session expired" | The saved sign-in no longer works and could not be renewed quietly. Press **Sign in** on the banner or the university card. |
| "The Blackboard session is too large to store securely" | The platform set more cookies than Windows Credential Manager can hold. Try signing in again; if it persists, report it with the log (cookie names only are logged). |
| "<function> is not enabled for the Moodle mobile service on this site" | The Moodle site does not expose a function the tracker needs (assignments, quizzes or calendar). Ask your university's Moodle administrators. |
| "A sync is already running" | Wait for it to finish; the Dashboard shows its progress. |
| "Sign in to a university first" | Sync only covers signed-in universities. Sign in on the Universities page. |
| "The sync was interrupted. Try again." | The app was closed or the sync stopped unexpectedly. Run **Sync** again. |
| A course or assignment is missing from the Dashboard | Check the course is ticked in **Edit courses**; press **Sync**; the item may be hidden (**Show hidden**) or removed from the platform (**Show removed**). |
| "Track at least one course before checking email" | Announcements needs at least one tracked course. Add a university and choose courses first. |
| Microsoft sign-in fails with "Your organization requires admin approval for this app" | The university tenant blocks user consent. Ask IT to grant admin consent, or use one of the fallbacks in [setup.md](setup.md#university-tenants-and-admin-consent). |
| "Set the Microsoft client ID in Settings" | Enter the Entra Application (client) ID in Settings and save. |
| "Signed in as X, expected Y" | You signed in with a different Microsoft account than the account's email. Use **Reconnect** (or **Sign in**) and pick the right one. |
| "Your sign-in expired. Reconnect the account", "Sign in to this account" or a Sign in banner | Press **Reconnect** (or **Sign in**) on the account. This also happens after you change the email address of a Microsoft 365 account. |
| IMAP test fails with an authentication error | Check host, username and password. Many providers (universities, Gmail, Outlook.com) require an **app password** or have IMAP disabled; create one in the provider's security settings. Only implicit TLS is supported (usually port 993). |
| IMAP connection error or timeout | Verify the host and port, your network and any VPN requirement of the university. |
| "The password is missing. Edit the account to set it" | Edit the IMAP account and enter the password. |
| AI errors: provider unavailable (Ollama) | Start Ollama and check the base URL in Settings. If the model is missing, run `ollama pull <model>` (default `qwen3:8b`) and make sure the model name in Settings matches. Use **Test provider** to confirm. |
| "Add your jev API key in Settings" (a check shows it as "The AI provider is not configured: Add your jev API key in Settings") | Choose Replace key in Settings and enter the key. |
| jev rejects the key (authentication error) | Replace the key. Rate limiting is retried automatically; if it persists, try again later. |
| A check will not start | "A check is already running": wait for the current run to finish. "There are no enabled accounts": enable or add one. "The AI provider is not configured: ...": fix the provider in Settings. |
| A run shows Failed with "Interrupted" | The app was closed or crashed during that run. On the next launch the run is marked interrupted; start a new check. Unprocessed messages are picked up again, because positions advance only after a batch is saved. |
| A run seems stuck on Running | Wait for the AI provider (jev requests time out after 60 s, Ollama after 120 s); local models on slow hardware can be slow. If nothing changes, restart the app; the run will be marked Interrupted. |
| Old messages are missing | The first sync covers only the account's first-sync days. Increase the value in the account form before its first check. |
