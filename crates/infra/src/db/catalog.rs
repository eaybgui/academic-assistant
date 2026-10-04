//! `CatalogRepository` implementation: the analyzer's read-only view of the LMS
//! universities and their tracked, available courses.

use academic_core::domain::{Catalog, Course, University};
use academic_core::ports::{CatalogRepository, RepoResult};

use super::util::map_err;
use super::Database;

impl CatalogRepository for Database {
    fn catalog(&self) -> RepoResult<Catalog> {
        let conn = self.conn()?;

        let mut uni_stmt = conn
            .prepare("SELECT id, name FROM universities ORDER BY name")
            .map_err(map_err)?;
        let universities = uni_stmt
            .query_map([], |row| {
                Ok(University {
                    id: row.get(0)?,
                    name: row.get(1)?,
                })
            })
            .map_err(map_err)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(map_err)?;

        let mut course_stmt = conn
            .prepare(
                "SELECT id, university_id, name, short_name FROM courses \
                 WHERE tracked = 1 AND available = 1 ORDER BY university_id, name",
            )
            .map_err(map_err)?;
        let courses = course_stmt
            .query_map([], |row| {
                let name: String = row.get(2)?;
                let short_name: Option<String> = row.get(3)?;
                Ok(Course {
                    id: row.get(0)?,
                    university_id: row.get(1)?,
                    aliases: aliases_for(&name, short_name),
                    name,
                })
            })
            .map_err(map_err)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(map_err)?;

        Ok(Catalog {
            universities,
            courses,
        })
    }
}

/// The short name is the course's only alias, when it adds information.
fn aliases_for(name: &str, short_name: Option<String>) -> Vec<String> {
    short_name
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty() && s != name)
        .into_iter()
        .collect()
}
