CREATE TABLE universities (
  id INTEGER PRIMARY KEY, name TEXT NOT NULL UNIQUE, created_at TEXT NOT NULL);
CREATE TABLE courses (
  id INTEGER PRIMARY KEY,
  university_id INTEGER NOT NULL REFERENCES universities(id) ON DELETE CASCADE,
  name TEXT NOT NULL, aliases TEXT NOT NULL DEFAULT '[]', created_at TEXT NOT NULL,
  UNIQUE (university_id, name));
CREATE TABLE accounts (
  id INTEGER PRIMARY KEY, name TEXT NOT NULL, email_address TEXT NOT NULL,
  provider_type TEXT NOT NULL CHECK (provider_type IN ('imap','microsoft_graph')),
  config TEXT NOT NULL, folders TEXT NOT NULL,
  default_university_id INTEGER REFERENCES universities(id) ON DELETE SET NULL,
  initial_sync_days INTEGER NOT NULL DEFAULT 14 CHECK (initial_sync_days BETWEEN 1 AND 365),
  enabled INTEGER NOT NULL DEFAULT 1, created_at TEXT NOT NULL, updated_at TEXT NOT NULL);
CREATE TABLE sync_states (
  account_id INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
  folder TEXT NOT NULL, cursor TEXT, last_run_at TEXT, last_success_at TEXT,
  status TEXT NOT NULL DEFAULT 'never' CHECK (status IN ('never','ok','error')),
  last_error TEXT, PRIMARY KEY (account_id, folder));
CREATE TABLE runs (
  id INTEGER PRIMARY KEY, trigger TEXT NOT NULL CHECK (trigger IN ('manual')),
  started_at TEXT NOT NULL, finished_at TEXT,
  status TEXT NOT NULL CHECK (status IN ('running','success','partial','failed')),
  accounts_processed INTEGER NOT NULL DEFAULT 0, messages_found INTEGER NOT NULL DEFAULT 0,
  messages_filtered INTEGER NOT NULL DEFAULT 0, messages_analyzed INTEGER NOT NULL DEFAULT 0,
  messages_failed INTEGER NOT NULL DEFAULT 0, results_created INTEGER NOT NULL DEFAULT 0,
  error_summary TEXT);
CREATE TABLE run_accounts (
  run_id INTEGER NOT NULL REFERENCES runs(id) ON DELETE CASCADE,
  account_id INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
  status TEXT NOT NULL CHECK (status IN ('success','failed')),
  messages_found INTEGER NOT NULL DEFAULT 0, messages_filtered INTEGER NOT NULL DEFAULT 0,
  messages_analyzed INTEGER NOT NULL DEFAULT 0, messages_failed INTEGER NOT NULL DEFAULT 0,
  results_created INTEGER NOT NULL DEFAULT 0,
  error_kind TEXT CHECK (error_kind IN ('auth','connection','protocol','ai','internal')),
  error_message TEXT, PRIMARY KEY (run_id, account_id));
CREATE TABLE processed_messages (
  id INTEGER PRIMARY KEY,
  account_id INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
  folder TEXT NOT NULL, provider_message_id TEXT NOT NULL, message_id_header TEXT,
  content_hash TEXT NOT NULL, received_at TEXT NOT NULL,
  outcome TEXT NOT NULL CHECK (outcome IN ('filtered','irrelevant','analyzed','failed')),
  reason TEXT, run_id INTEGER NOT NULL REFERENCES runs(id), processed_at TEXT NOT NULL,
  UNIQUE (account_id, folder, provider_message_id));
CREATE INDEX idx_processed_msgid ON processed_messages (account_id, message_id_header);
CREATE INDEX idx_processed_hash ON processed_messages (account_id, content_hash);
CREATE TABLE ai_results (
  id INTEGER PRIMARY KEY,
  account_id INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
  processed_message_id INTEGER NOT NULL UNIQUE REFERENCES processed_messages(id) ON DELETE CASCADE,
  run_id INTEGER NOT NULL REFERENCES runs(id),
  detected_at TEXT NOT NULL, received_at TEXT NOT NULL,
  subject TEXT NOT NULL, sender_name TEXT, sender_email TEXT NOT NULL,
  university_id INTEGER REFERENCES universities(id) ON DELETE SET NULL,
  course_id INTEGER REFERENCES courses(id) ON DELETE SET NULL,
  category TEXT NOT NULL CHECK (category IN ('notice','task','exam','event','change','general_info')),
  priority TEXT NOT NULL CHECK (priority IN ('low','medium','high','urgent')),
  action_required INTEGER NOT NULL, possible_change INTEGER NOT NULL, confidence REAL,
  model_provider TEXT NOT NULL, model_name TEXT NOT NULL, source_url TEXT,
  seen INTEGER NOT NULL DEFAULT 0, seen_at TEXT);
CREATE INDEX idx_results_seen ON ai_results (seen, received_at);
CREATE TABLE result_dates (
  id INTEGER PRIMARY KEY,
  result_id INTEGER NOT NULL REFERENCES ai_results(id) ON DELETE CASCADE,
  date TEXT NOT NULL, time TEXT,
  kind TEXT NOT NULL CHECK (kind IN ('exam','deadline','event','class','other')));
CREATE TABLE settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);
