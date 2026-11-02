-- Phase 3: tasks the user creates for a calendar day. `date` is a local YYYY-MM-DD, never an instant.
CREATE TABLE personal_tasks (
  id INTEGER PRIMARY KEY,
  title TEXT NOT NULL,
  date TEXT NOT NULL,
  note TEXT,
  done INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL);
CREATE INDEX idx_personal_tasks_date ON personal_tasks (date);
