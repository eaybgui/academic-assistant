use serde::{Deserialize, Serialize};

/// A university the user attends, used to scope courses and default an account.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct University {
    pub id: i64,
    pub name: String,
}

/// A course within a university. `aliases` lists alternate names/codes the analyzer
/// should also recognize (e.g. a short code alongside the full course name).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Course {
    pub id: i64,
    pub university_id: i64,
    pub name: String,
    pub aliases: Vec<String>,
}

/// The full set of universities and courses the user has configured.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Catalog {
    pub universities: Vec<University>,
    pub courses: Vec<Course>,
}
