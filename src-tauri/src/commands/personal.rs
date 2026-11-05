//! Commands: the user's personal tasks (own entries on the calendar).

use tauri::State;

use super::logged;
use crate::dto::{PersonalTaskDto, PersonalTaskInput};
use crate::error::CommandError;
use crate::state::AppState;
use crate::validation;

#[tauri::command]
pub async fn create_personal_task(
    state: State<'_, AppState>,
    input: PersonalTaskInput,
) -> Result<PersonalTaskDto, CommandError> {
    logged("create_personal_task", create(&state, input))
}

#[tauri::command]
pub async fn update_personal_task(
    state: State<'_, AppState>,
    id: i64,
    input: PersonalTaskInput,
) -> Result<PersonalTaskDto, CommandError> {
    logged("update_personal_task", update(&state, id, input))
}

#[tauri::command]
pub async fn set_personal_task_done(
    state: State<'_, AppState>,
    id: i64,
    value: bool,
) -> Result<PersonalTaskDto, CommandError> {
    logged("set_personal_task_done", set_done(&state, id, value))
}

#[tauri::command]
pub async fn delete_personal_task(state: State<'_, AppState>, id: i64) -> Result<(), CommandError> {
    logged("delete_personal_task", delete(&state, id))
}

fn create(state: &AppState, input: PersonalTaskInput) -> Result<PersonalTaskDto, CommandError> {
    let draft = validation::personal_task(input)?;
    let task = state
        .personal_tasks
        .insert_personal_task(&draft, state.clock.now())?;
    Ok(PersonalTaskDto::from(&task))
}

fn update(
    state: &AppState,
    id: i64,
    input: PersonalTaskInput,
) -> Result<PersonalTaskDto, CommandError> {
    let draft = validation::personal_task(input)?;
    let task = state
        .personal_tasks
        .update_personal_task(id, &draft, state.clock.now())?;
    Ok(PersonalTaskDto::from(&task))
}

fn set_done(state: &AppState, id: i64, done: bool) -> Result<PersonalTaskDto, CommandError> {
    let task = state
        .personal_tasks
        .set_personal_task_done(id, done, state.clock.now())?;
    Ok(PersonalTaskDto::from(&task))
}

fn delete(state: &AppState, id: i64) -> Result<(), CommandError> {
    Ok(state.personal_tasks.delete_personal_task(id)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::CommandErrorKind;
    use crate::test_support::harness;
    use chrono::NaiveDate;

    fn input(title: &str, date: &str, note: Option<&str>) -> PersonalTaskInput {
        PersonalTaskInput {
            title: title.into(),
            date: date.into(),
            note: note.map(str::to_string),
        }
    }

    fn day(year: i32, month: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(year, month, day).unwrap()
    }

    fn validation_message(result: Result<PersonalTaskDto, CommandError>) -> String {
        let error = result.unwrap_err();
        assert_eq!(error.kind, CommandErrorKind::Validation);
        error.message
    }

    #[test]
    fn create_trims_and_returns_the_task() {
        let h = harness();
        let task = create(
            &h.state,
            input("  Go to the doctor  ", "2026-11-05", Some("  ")),
        )
        .unwrap();
        assert_eq!(task.title, "Go to the doctor");
        assert_eq!(task.date, day(2026, 11, 5));
        assert_eq!(task.note, None);
        assert!(!task.done);
    }

    #[test]
    fn create_rejects_a_blank_title() {
        let h = harness();
        assert_eq!(
            validation_message(create(&h.state, input("   ", "2026-11-05", None))),
            "Enter a title"
        );
    }

    #[test]
    fn create_rejects_a_201_character_title_but_accepts_200() {
        let h = harness();
        assert_eq!(
            validation_message(create(
                &h.state,
                input(&"ñ".repeat(201), "2026-11-05", None)
            )),
            "Titles can have at most 200 characters"
        );
        let title = "ñ".repeat(200);
        assert_eq!(
            create(&h.state, input(&title, "2026-11-05", None))
                .unwrap()
                .title,
            title
        );
    }

    #[test]
    fn create_rejects_a_bad_day() {
        let h = harness();
        for bad in ["2026-02-30", "05/11/2026"] {
            assert_eq!(
                validation_message(create(&h.state, input("Go to the doctor", bad, None))),
                "Enter a valid day"
            );
        }
    }

    #[test]
    fn create_rejects_a_too_long_note() {
        let h = harness();
        assert_eq!(
            validation_message(create(
                &h.state,
                input("Go to the doctor", "2026-11-05", Some(&"x".repeat(2001)))
            )),
            "Notes can have at most 2000 characters"
        );
    }

    #[test]
    fn update_and_done_and_delete_round_trip() {
        let h = harness();
        let created = create(&h.state, input("Go to the doctor", "2026-11-05", None)).unwrap();

        let updated = update(
            &h.state,
            created.id,
            input(" Call the bank ", "2026-11-06", Some("Before noon")),
        )
        .unwrap();
        assert_eq!(updated.id, created.id);
        assert_eq!(updated.title, "Call the bank");
        assert_eq!(updated.date, day(2026, 11, 6));
        assert_eq!(updated.note.as_deref(), Some("Before noon"));

        assert!(set_done(&h.state, created.id, true).unwrap().done);
        assert!(!set_done(&h.state, created.id, false).unwrap().done);

        delete(&h.state, created.id).unwrap();
        assert_eq!(
            delete(&h.state, created.id).unwrap_err().kind,
            CommandErrorKind::NotFound
        );
    }

    #[test]
    fn a_missing_id_is_not_found() {
        let h = harness();
        assert_eq!(
            update(
                &h.state,
                9999,
                input("Go to the doctor", "2026-11-05", None)
            )
            .unwrap_err()
            .kind,
            CommandErrorKind::NotFound
        );
        assert_eq!(
            set_done(&h.state, 9999, true).unwrap_err().kind,
            CommandErrorKind::NotFound
        );
    }

    #[test]
    fn the_dto_serializes_in_the_frontend_shape() {
        let h = harness();
        let task = create(
            &h.state,
            input("Go to the doctor", "2026-11-05", Some("Room 4")),
        )
        .unwrap();
        assert_eq!(
            serde_json::to_value(&task).unwrap(),
            serde_json::json!({
                "id": task.id,
                "title": "Go to the doctor",
                "date": "2026-11-05",
                "note": "Room 4",
                "done": false
            })
        );
    }
}
