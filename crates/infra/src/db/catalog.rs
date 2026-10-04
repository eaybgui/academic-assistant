//! `CatalogRepository` implementation.

use academic_core::domain::{Catalog, Course, University};
use academic_core::ports::{CatalogRepository, RepoError, RepoResult};
use chrono::{DateTime, Utc};
use rusqlite::{params, OptionalExtension};

use super::util::{map_err, parse_json, to_json, to_rfc3339};
use super::Database;

fn read_university(row: &rusqlite::Row) -> rusqlite::Result<University> {
    Ok(University {
        id: row.get(0)?,
        name: row.get(1)?,
    })
}

fn read_course(row: &rusqlite::Row) -> rusqlite::Result<(i64, i64, String, String)> {
    Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
}

impl CatalogRepository for Database {
    fn catalog(&self) -> RepoResult<Catalog> {
        let conn = self.conn()?;

        let mut uni_stmt = conn
            .prepare("SELECT id, name FROM universities ORDER BY name")
            .map_err(map_err)?;
        let universities = uni_stmt
            .query_map([], read_university)
            .map_err(map_err)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(map_err)?;

        let mut course_stmt = conn
            .prepare(
                "SELECT id, university_id, name, aliases FROM courses ORDER BY university_id, name",
            )
            .map_err(map_err)?;
        let raw_courses = course_stmt
            .query_map([], read_course)
            .map_err(map_err)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(map_err)?;
        let mut courses = Vec::with_capacity(raw_courses.len());
        for (id, university_id, name, aliases_json) in raw_courses {
            courses.push(Course {
                id,
                university_id,
                name,
                aliases: parse_json(&aliases_json)?,
            });
        }

        Ok(Catalog {
            universities,
            courses,
        })
    }

    fn add_university(&self, name: &str, now: DateTime<Utc>) -> RepoResult<University> {
        let conn = self.conn()?;
        conn.execute(
            "INSERT INTO universities (name, created_at) VALUES (?1, ?2)",
            params![name, to_rfc3339(now)],
        )
        .map_err(map_err)?;
        let id = conn.last_insert_rowid();
        Ok(University {
            id,
            name: name.to_string(),
        })
    }

    fn rename_university(&self, id: i64, name: &str) -> RepoResult<University> {
        let conn = self.conn()?;
        let changed = conn
            .execute(
                "UPDATE universities SET name = ?1 WHERE id = ?2",
                params![name, id],
            )
            .map_err(map_err)?;
        if changed == 0 {
            return Err(RepoError::NotFound);
        }
        Ok(University {
            id,
            name: name.to_string(),
        })
    }

    fn delete_university(&self, id: i64) -> RepoResult<()> {
        let conn = self.conn()?;
        let changed = conn
            .execute("DELETE FROM universities WHERE id = ?1", params![id])
            .map_err(map_err)?;
        if changed == 0 {
            return Err(RepoError::NotFound);
        }
        Ok(())
    }

    fn add_course(
        &self,
        university_id: i64,
        name: &str,
        aliases: &[String],
        now: DateTime<Utc>,
    ) -> RepoResult<Course> {
        let aliases_json = to_json(aliases)?;
        let conn = self.conn()?;
        conn.execute(
            "INSERT INTO courses (university_id, name, aliases, created_at) VALUES (?1, ?2, ?3, ?4)",
            params![university_id, name, aliases_json, to_rfc3339(now)],
        )
        .map_err(map_err)?;
        let id = conn.last_insert_rowid();
        Ok(Course {
            id,
            university_id,
            name: name.to_string(),
            aliases: aliases.to_vec(),
        })
    }

    fn update_course(&self, id: i64, name: &str, aliases: &[String]) -> RepoResult<Course> {
        let aliases_json = to_json(aliases)?;
        let conn = self.conn()?;
        let university_id: Option<i64> = conn
            .query_row(
                "SELECT university_id FROM courses WHERE id = ?1",
                params![id],
                |row| row.get(0),
            )
            .optional()
            .map_err(map_err)?;
        let Some(university_id) = university_id else {
            return Err(RepoError::NotFound);
        };
        conn.execute(
            "UPDATE courses SET name = ?1, aliases = ?2 WHERE id = ?3",
            params![name, aliases_json, id],
        )
        .map_err(map_err)?;
        Ok(Course {
            id,
            university_id,
            name: name.to_string(),
            aliases: aliases.to_vec(),
        })
    }

    fn delete_course(&self, id: i64) -> RepoResult<()> {
        let conn = self.conn()?;
        let changed = conn
            .execute("DELETE FROM courses WHERE id = ?1", params![id])
            .map_err(map_err)?;
        if changed == 0 {
            return Err(RepoError::NotFound);
        }
        Ok(())
    }
}
