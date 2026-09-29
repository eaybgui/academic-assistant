//! Commands: catalog CRUD (universities and courses).

use tauri::State;

use super::logged;
use crate::dto::{CourseDto, UniversityDto};
use crate::error::CommandError;
use crate::state::AppState;
use crate::validation;

#[tauri::command]
pub async fn get_catalog(state: State<'_, AppState>) -> Result<Vec<UniversityDto>, CommandError> {
    logged("get_catalog", catalog(&state))
}

#[tauri::command]
pub async fn add_university(
    state: State<'_, AppState>,
    name: String,
) -> Result<UniversityDto, CommandError> {
    logged("add_university", add_university_impl(&state, &name))
}

#[tauri::command]
pub async fn update_university(
    state: State<'_, AppState>,
    id: i64,
    name: String,
) -> Result<UniversityDto, CommandError> {
    logged("update_university", rename_university(&state, id, &name))
}

#[tauri::command]
pub async fn remove_university(state: State<'_, AppState>, id: i64) -> Result<(), CommandError> {
    logged("remove_university", delete_university(&state, id))
}

#[tauri::command]
pub async fn add_course(
    state: State<'_, AppState>,
    university_id: i64,
    name: String,
    aliases: Vec<String>,
) -> Result<CourseDto, CommandError> {
    logged(
        "add_course",
        add_course_impl(&state, university_id, &name, aliases),
    )
}

#[tauri::command]
pub async fn update_course(
    state: State<'_, AppState>,
    id: i64,
    name: String,
    aliases: Vec<String>,
) -> Result<CourseDto, CommandError> {
    logged(
        "update_course",
        update_course_impl(&state, id, &name, aliases),
    )
}

#[tauri::command]
pub async fn remove_course(state: State<'_, AppState>, id: i64) -> Result<(), CommandError> {
    logged("remove_course", delete_course(&state, id))
}

fn catalog(state: &AppState) -> Result<Vec<UniversityDto>, CommandError> {
    Ok(UniversityDto::from_catalog(&state.catalog.catalog()?))
}

fn add_university_impl(state: &AppState, name: &str) -> Result<UniversityDto, CommandError> {
    let name = validation::name(name)?;
    let university = state.catalog.add_university(&name, state.clock.now())?;
    Ok(UniversityDto::new(&university, &[]))
}

fn rename_university(state: &AppState, id: i64, name: &str) -> Result<UniversityDto, CommandError> {
    let name = validation::name(name)?;
    let university = state.catalog.rename_university(id, &name)?;
    let catalog = state.catalog.catalog()?;
    Ok(UniversityDto::new(&university, &catalog.courses))
}

fn add_course_impl(
    state: &AppState,
    university_id: i64,
    name: &str,
    aliases: Vec<String>,
) -> Result<CourseDto, CommandError> {
    let name = validation::name(name)?;
    let aliases = validation::non_blank(aliases);
    let course = state
        .catalog
        .add_course(university_id, &name, &aliases, state.clock.now())?;
    Ok(CourseDto::from(&course))
}

fn update_course_impl(
    state: &AppState,
    id: i64,
    name: &str,
    aliases: Vec<String>,
) -> Result<CourseDto, CommandError> {
    let name = validation::name(name)?;
    let aliases = validation::non_blank(aliases);
    Ok(CourseDto::from(
        &state.catalog.update_course(id, &name, &aliases)?,
    ))
}

fn delete_university(state: &AppState, id: i64) -> Result<(), CommandError> {
    Ok(state.catalog.delete_university(id)?)
}

fn delete_course(state: &AppState, id: i64) -> Result<(), CommandError> {
    Ok(state.catalog.delete_course(id)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::CommandErrorKind;
    use crate::test_support::harness;

    #[test]
    fn university_names_are_validated_and_trimmed() {
        let h = harness();
        assert_eq!(
            add_university_impl(&h.state, "   ").unwrap_err().message,
            "Enter a name"
        );
        let university = add_university_impl(&h.state, "  UV ").unwrap();
        assert_eq!(university.name, "UV");
        assert!(university.courses.is_empty());
    }

    #[test]
    fn duplicate_university_is_a_conflict() {
        let h = harness();
        add_university_impl(&h.state, "UV").unwrap();
        let error = add_university_impl(&h.state, "UV").unwrap_err();
        assert_eq!(error.kind, CommandErrorKind::Conflict);
    }

    #[test]
    fn renaming_keeps_the_courses() {
        let h = harness();
        let university = add_university_impl(&h.state, "UV").unwrap();
        add_course_impl(&h.state, university.id, "Algebra", vec![]).unwrap();

        let renamed = rename_university(&h.state, university.id, "Valencia").unwrap();
        assert_eq!(renamed.name, "Valencia");
        assert_eq!(renamed.courses.len(), 1);
    }

    #[test]
    fn courses_trim_aliases_and_nest_in_the_catalog() {
        let h = harness();
        let university = add_university_impl(&h.state, "UV").unwrap();
        let course = add_course_impl(
            &h.state,
            university.id,
            " Algebra ",
            vec![" ALG ".into(), "".into(), "Álgebra".into()],
        )
        .unwrap();
        assert_eq!(course.name, "Algebra");
        assert_eq!(course.aliases, vec!["ALG", "Álgebra"]);

        let updated =
            update_course_impl(&h.state, course.id, "Algebra II", vec!["A2".into()]).unwrap();
        assert_eq!(updated.aliases, vec!["A2"]);

        let catalog = catalog(&h.state).unwrap();
        assert_eq!(catalog[0].courses[0].name, "Algebra II");
    }

    #[test]
    fn course_under_a_missing_university_is_not_found() {
        let h = harness();
        let error = add_course_impl(&h.state, 404, "Algebra", vec![]).unwrap_err();
        assert_eq!(error.kind, CommandErrorKind::NotFound);
    }

    #[test]
    fn deleting_a_university_removes_its_courses() {
        let h = harness();
        let university = add_university_impl(&h.state, "UV").unwrap();
        add_course_impl(&h.state, university.id, "Algebra", vec![]).unwrap();
        delete_university(&h.state, university.id).unwrap();
        assert!(catalog(&h.state).unwrap().is_empty());
    }
}
