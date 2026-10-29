//! `CourseRepository` implementation.

use academic_core::domain::{LmsCourse, RemoteCourse, UniversityId};
use academic_core::ports::{CourseRepository, RepoError, RepoResult};
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};

use super::util::{map_err, parse_datetime_opt, to_rfc3339};
use super::Database;

fn query_courses(conn: &Connection, university_id: UniversityId) -> RepoResult<Vec<LmsCourse>> {
    let mut stmt = conn
        .prepare(
            "SELECT id, university_id, external_id, name, short_name, url, tracked, available, \
             reviewed, items_synced_at FROM courses WHERE university_id = ?1 ORDER BY name, id",
        )
        .map_err(map_err)?;
    let rows = stmt
        .query_map([university_id], |row| {
            Ok((
                LmsCourse {
                    id: row.get(0)?,
                    university_id: row.get(1)?,
                    external_id: row.get(2)?,
                    name: row.get(3)?,
                    short_name: row.get(4)?,
                    url: row.get(5)?,
                    tracked: row.get(6)?,
                    available: row.get(7)?,
                    reviewed: row.get(8)?,
                    items_synced_at: None,
                },
                row.get::<_, Option<String>>(9)?,
            ))
        })
        .map_err(map_err)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(map_err)?;
    rows.into_iter()
        .map(|(mut course, synced)| {
            course.items_synced_at = parse_datetime_opt(synced)?;
            Ok(course)
        })
        .collect()
}

fn university_exists(conn: &Connection, id: UniversityId) -> RepoResult<bool> {
    conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM universities WHERE id = ?1)",
        [id],
        |row| row.get(0),
    )
    .map_err(map_err)
}

impl CourseRepository for Database {
    fn list_courses(&self, university_id: UniversityId) -> RepoResult<Vec<LmsCourse>> {
        let conn = self.conn()?;
        query_courses(&conn, university_id)
    }

    fn upsert_remote_courses(
        &self,
        university_id: UniversityId,
        courses: &[RemoteCourse],
        now: DateTime<Utc>,
    ) -> RepoResult<()> {
        let mut conn = self.conn()?;
        let tx = conn.transaction().map_err(map_err)?;
        if !university_exists(&tx, university_id)? {
            return Err(RepoError::NotFound);
        }
        tx.execute(
            "UPDATE courses SET available = 0 WHERE university_id = ?1",
            [university_id],
        )
        .map_err(map_err)?;
        for course in courses {
            tx.execute(
                "INSERT INTO courses (university_id, external_id, name, short_name, url, created_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6) \
                 ON CONFLICT (university_id, external_id) DO UPDATE SET \
                 name = excluded.name, short_name = excluded.short_name, \
                 url = excluded.url, available = 1",
                params![
                    university_id,
                    course.external_id,
                    course.name,
                    course.short_name,
                    course.url,
                    to_rfc3339(now)
                ],
            )
            .map_err(map_err)?;
        }
        tx.commit().map_err(map_err)
    }

    fn set_tracked_courses(
        &self,
        university_id: UniversityId,
        course_ids: &[i64],
    ) -> RepoResult<Vec<LmsCourse>> {
        let mut conn = self.conn()?;
        let tx = conn.transaction().map_err(map_err)?;
        tx.execute(
            "UPDATE courses SET tracked = 0, reviewed = 1 WHERE university_id = ?1",
            [university_id],
        )
        .map_err(map_err)?;
        for id in course_ids {
            let changed = tx
                .execute(
                    "UPDATE courses SET tracked = 1 WHERE id = ?1 AND university_id = ?2",
                    params![id, university_id],
                )
                .map_err(map_err)?;
            if changed == 0 {
                return Err(RepoError::NotFound); // dropping `tx` rolls back
            }
        }
        tx.commit().map_err(map_err)?;
        query_courses(&conn, university_id)
    }
}
