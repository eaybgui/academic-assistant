-- Phase 2: deliverables, exams and course events discovered on LMS platforms.
CREATE TABLE lms_items (
  id INTEGER PRIMARY KEY,
  university_id INTEGER NOT NULL REFERENCES universities(id) ON DELETE CASCADE,
  course_id INTEGER NOT NULL REFERENCES courses(id) ON DELETE CASCADE,
  external_id TEXT NOT NULL,
  kind TEXT NOT NULL CHECK (kind IN ('assignment','quiz','test','event','other')),
  title TEXT NOT NULL,
  due_at TEXT,
  url TEXT,
  platform_status TEXT NOT NULL
    CHECK (platform_status IN ('not_submitted','submitted','graded','not_applicable')),
  grade_display TEXT, grade_score REAL, grade_max REAL,
  change_kind TEXT CHECK (change_kind IN ('new','due_changed')),
  previous_due_at TEXT,
  removed_at TEXT,
  done INTEGER NOT NULL DEFAULT 0,
  hidden INTEGER NOT NULL DEFAULT 0,
  note TEXT,
  first_seen_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  UNIQUE (university_id, external_id));
CREATE INDEX idx_lms_items_due ON lms_items (university_id, due_at);
