//! Catalog DTOs: universities with their nested courses.

use academic_core::domain::{Catalog, Course, University};
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CourseDto {
    pub id: i64,
    pub university_id: i64,
    pub name: String,
    pub aliases: Vec<String>,
}

impl From<&Course> for CourseDto {
    fn from(course: &Course) -> Self {
        Self {
            id: course.id,
            university_id: course.university_id,
            name: course.name.clone(),
            aliases: course.aliases.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct UniversityDto {
    pub id: i64,
    pub name: String,
    pub courses: Vec<CourseDto>,
}

impl UniversityDto {
    pub fn new(university: &University, courses: &[Course]) -> Self {
        Self {
            id: university.id,
            name: university.name.clone(),
            courses: courses
                .iter()
                .filter(|course| course.university_id == university.id)
                .map(CourseDto::from)
                .collect(),
        }
    }

    /// Groups the flat catalog into universities with their courses, keeping the
    /// repository's ordering for both.
    pub fn from_catalog(catalog: &Catalog) -> Vec<Self> {
        catalog
            .universities
            .iter()
            .map(|university| Self::new(university, &catalog.courses))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn course(id: i64, university_id: i64, name: &str) -> Course {
        Course {
            id,
            university_id,
            name: name.into(),
            aliases: vec!["ALG".into()],
        }
    }

    fn catalog() -> Catalog {
        Catalog {
            universities: vec![
                University {
                    id: 1,
                    name: "UV".into(),
                },
                University {
                    id: 2,
                    name: "UAH".into(),
                },
            ],
            courses: vec![course(10, 1, "Algebra"), course(11, 1, "Calculus")],
        }
    }

    #[test]
    fn groups_courses_under_their_university() {
        let universities = UniversityDto::from_catalog(&catalog());
        assert_eq!(universities.len(), 2);
        assert_eq!(universities[0].courses.len(), 2);
        assert!(universities[1].courses.is_empty());
    }

    #[test]
    fn university_dto_matches_the_typescript_shape() {
        let universities = UniversityDto::from_catalog(&catalog());
        assert_eq!(
            serde_json::to_value(&universities[0]).unwrap(),
            json!({
                "id": 1,
                "name": "UV",
                "courses": [
                    { "id": 10, "universityId": 1, "name": "Algebra", "aliases": ["ALG"] },
                    { "id": 11, "universityId": 1, "name": "Calculus", "aliases": ["ALG"] }
                ]
            })
        );
    }
}
