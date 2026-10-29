-- Phase 2: universities become LMS connections; courses are discovered from the
-- platform. The manual catalog is discarded: deleting the rows lets the existing
-- foreign keys null out ai_results.university_id/course_id and
-- accounts.default_university_id before the tables are recreated.
DELETE FROM courses;
DELETE FROM universities;
DROP TABLE courses;
DROP TABLE universities;

CREATE TABLE universities (
  id INTEGER PRIMARY KEY,
  name TEXT NOT NULL UNIQUE,
  platform TEXT NOT NULL CHECK (platform IN ('moodle','blackboard')),
  base_url TEXT NOT NULL UNIQUE,
  login_state TEXT NOT NULL DEFAULT 'never'
    CHECK (login_state IN ('never','ok','expired','error')),
  account_label TEXT,
  last_sync_at TEXT,
  last_sync_outcome TEXT CHECK (last_sync_outcome IN ('success','failed','login_required')),
  last_sync_error TEXT,
  created_at TEXT NOT NULL);

CREATE TABLE courses (
  id INTEGER PRIMARY KEY,
  university_id INTEGER NOT NULL REFERENCES universities(id) ON DELETE CASCADE,
  external_id TEXT NOT NULL,
  name TEXT NOT NULL,
  short_name TEXT,
  url TEXT,
  tracked INTEGER NOT NULL DEFAULT 0,
  available INTEGER NOT NULL DEFAULT 1,
  reviewed INTEGER NOT NULL DEFAULT 0,
  items_synced_at TEXT,
  created_at TEXT NOT NULL,
  UNIQUE (university_id, external_id));
