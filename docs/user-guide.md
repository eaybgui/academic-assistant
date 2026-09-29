# User guide

This guide describes what you can do in Academic Assistant and what happens to your data. If
you have not installed or configured anything yet, start with [setup.md](setup.md).

## First-time checklist

1. **Settings**: pick the AI provider. With **Ollama**, make sure Ollama is running and the
   model is pulled (`ollama pull qwen3:8b`). With **jev**, use Replace key to enter your API
   key. For Microsoft 365, enter the Entra client ID.
2. **Catalog** (optional but recommended): add your universities and courses, with aliases
   such as course codes. The AI uses them to name the course of each email.
3. **Accounts**: add each mailbox and press **Test**.
4. **Dashboard**: press **Check now**.

## The screens

The left sidebar links to Dashboard, Results (with a badge showing the number of unseen
results), Runs, Accounts, Catalog and Settings.

### Dashboard

- **Last check**: time and status of the most recent run, for example "partial: 1 of 2
  accounts failed".
- **Check now**: starts a manual check of all enabled accounts. While it runs, the button is
  replaced by a progress label such as `UV IMAP: Analyzing 12/40`. You can navigate to other
  screens meanwhile; the run continues.
- **Attention banners**: one per enabled account that needs action, for example an account
  whose last sync failed, or one that has no stored password or Microsoft sign-in.
- **Counters** of unseen results: New, Tasks, Exams, Events, and Changes and notices.
- **New results**: every unseen result, sorted by nearest upcoming date (default), priority or
  received date. **Mark all as seen** clears the list.
- **Result card**: subject and priority badge; sender, course (university), account and
  received date; category; the dates found, each with its kind (exam, deadline, event, class,
  other); the tags **Action required** and **Possible change**; **Open email** (Microsoft 365
  results, opens the message in your browser) or **Copy Message-ID** (IMAP results, which have no
  link); and **Seen**.

### Results

The full history. Use the search box (subject, sender name and address) and the **Filters and
sort** button, which opens a modal with state (new/seen), category, account, course and sort
order; a badge shows how many filters are active. Apply commits your choices, Reset clears them.
Rows show subject, category, next date and priority, with a dot for unseen results. Click a
subject to expand the row: sender with address, account, received and detected dates, all
dates, flags, **Open email** / **Copy Message-ID**, and **Mark as seen** / **Mark as new**
(undo). Results are paged 25 per page.

### Runs

History of checks, newest first: status (Running, Success, Partial, Failed), start time,
duration, trigger (Manual) and counters (found, filtered, analyzed, failed, results). Expand a
run to see its error summary and each account's status; a failed account shows an error kind
(authentication, connection, protocol, AI, internal) and a message.

### Accounts

One row per account: provider, email, default university, folders and last sync status.

- **Add account**: switch between **IMAP** and **Microsoft 365**.
  - IMAP fields: display name, email, host, port (default 993, implicit TLS), username,
    password, folders (default `INBOX`), default university, first-sync days.
  - Microsoft 365 fields: display name, email, folders (default `inbox`), default university,
    first-sync days. After saving, press **Sign in with Microsoft**; your browser opens, you
    sign in, and the app receives the result on a local loopback address (5 minute limit). The
    account you sign in with must match the email address entered, otherwise nothing is stored.
  - The provider type cannot be changed after creation. When editing an IMAP account, leave
    the password empty to keep the stored one.
  - Secrets are saved in Windows Credential Manager, never in the database.
- **Test**: checks the connection and shows the result inline.
- **Edit**, **Enabled** toggle (disabled accounts are skipped by checks).
- **Reconnect** / **Sign in** (Microsoft 365 only): repeat the sign-in, for example when it
  expired.
- **Delete**: removes the account, its stored secrets and **all its results**. Disable the
  account instead to keep its history.

### Catalog

Universities and their courses. Each course has a name and optional aliases (comma separated;
codes and abbreviations work well). Deleting a university deletes its courses; accounts that
used it as default keep working without a default university.

### Settings

- **AI provider**: **Ollama** ("Stays on this PC") or **jev** ("Sends email text to
  TypeSafe"). The default is Ollama.
- **jev**: API key (write-only: it shows only "Configured" or "Not configured", with Replace
  key and Remove key), and the model name (default `jev-latest`).
- **Ollama**: base URL (default `http://localhost:11434`) and model (default `qwen3:8b`).
- **Test provider**: asks the selected provider one fixed question and reports the result.
- **Microsoft Entra client ID**: needed for Microsoft 365 accounts ([setup.md](setup.md)).
- **Blocked senders and domains**: one per line. An entry starting with `@` (for example
  `@newsletter.example.com`) blocks that exact domain (not its subdomains); any other entry
  blocks that exact address. Matching is case-insensitive. Blocked messages are discarded
  before the AI sees them.

Press Save to apply the whole form.

## How a check works

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
4. Relevant messages become results on the Dashboard and in Results.
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
  time, a new room, a cancellation, new instructions). In Phase 1 the app only flags this; it
  does not link the message to the earlier one or compute what changed.
- **Dates**: the app finds date mentions (Spanish and English formats, plus "today",
  "tomorrow", "next Monday") and the AI only classifies what each one refers to. Dates the AI
  judges unrelated are dropped.

## Privacy

- **Stored on your PC** (SQLite): accounts (without secrets), catalog, settings, run history,
  message identifiers and a content hash for deduplication, and for each relevant message its
  subject, sender, category, priority, dates, flags, chosen course and university, and the model
  used. **Email bodies are never stored.**
- **Stored in Windows Credential Manager**: IMAP passwords, Microsoft refresh tokens and the
  jev API key.
- **Read-only access**: the app never marks messages as read, moves, deletes or sends.
- **Sent to jev** (only if selected): for each message that passed the filters, the subject,
  sender name and address, received time, the cleaned body (quotes and signatures removed, at
  most 12,000 characters), the account's default university name, and your catalog's course
  names and aliases. With **Ollama**, the same content is sent only to your local Ollama server.
- The application log contains ids, counters and sanitized errors, never message content or
  secrets.

## Troubleshooting

| Symptom | What to do |
|---|---|
| Microsoft sign-in fails with "Your organization requires admin approval for this app" | The university tenant blocks user consent. Ask IT to grant admin consent, or use one of the fallbacks in [setup.md](setup.md#university-tenants-and-admin-consent). |
| "Set the Microsoft client ID in Settings" | Enter the Entra Application (client) ID in Settings and save. |
| "Signed in as X, expected Y" | You signed in with a different Microsoft account than the account's email. Use **Reconnect** and pick the right one. |
| "Your sign-in expired. Reconnect the account" or a Sign in banner | Press **Reconnect** on the account. |
| IMAP test fails with an authentication error | Check host, username and password. Many providers (universities, Gmail, Outlook.com) require an **app password** or have IMAP disabled; create one in the provider's security settings. Only implicit TLS is supported (usually port 993). |
| IMAP connection error or timeout | Verify the host and port, your network and any VPN requirement of the university. |
| "The password is missing. Edit the account to set it" | Edit the IMAP account and enter the password. |
| AI errors: provider unavailable (Ollama) | Start Ollama and check the base URL in Settings. If the model is missing, run `ollama pull <model>` (default `qwen3:8b`) and make sure the model name in Settings matches. Use **Test provider** to confirm. |
| "Add your jev API key in Settings" | Choose Replace key in Settings and enter the key. |
| jev rejects the key (authentication error) | Replace the key. Rate limiting is retried automatically; if it persists, try again later. |
| A check will not start | "Already running": wait for the current run to finish. "No enabled accounts": enable or add one. AI not configured: fix the provider in Settings. |
| A run shows Failed with "Interrupted" | The app was closed or crashed during that run. On the next launch the run is marked interrupted; start a new check. Unprocessed messages are picked up again, because positions advance only after a batch is saved. |
| A run seems stuck on Running | Wait for the AI provider (jev requests time out after 60 s, Ollama after 120 s); local models on slow hardware can be slow. If nothing changes, restart the app; the run will be marked Interrupted. |
| Old messages are missing | The first sync covers only the account's first-sync days. Increase the value in the account form before its first check. |
