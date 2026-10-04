//! Commands: LMS universities, sign-in and session check (spec §9.2, §18.4).

use academic_core::domain::{
    lms_credential_key, LmsPlatform, LmsUniversity, LoginState, UniversityId,
};
use academic_core::lms::{adopt_credential, ensure_session, normalize_base_url, SessionError};
use academic_core::ports::LmsError;
use academic_infra::lms::probe::{probe, ProbeError};
use tauri::{AppHandle, State};

use super::logged;
use crate::dto::{LmsUniversityDto, LmsUniversityInput, ProbeDto};
use crate::error::CommandError;
use crate::lms_auth::{capture_credential, LoginMode};
use crate::state::AppState;
use crate::validation;

const DUPLICATE_PLATFORM: &str = "This platform is already added";
const NOT_FOUND: &str = "University not found";

#[tauri::command]
pub async fn lms_probe(
    state: State<'_, AppState>,
    url: String,
    platform: LmsPlatform,
) -> Result<ProbeDto, CommandError> {
    logged("lms_probe", probe_url(&state, &url, platform).await)
}

#[tauri::command]
pub async fn get_universities(
    state: State<'_, AppState>,
) -> Result<Vec<LmsUniversityDto>, CommandError> {
    logged("get_universities", universities(&state))
}

#[tauri::command]
pub async fn add_university(
    state: State<'_, AppState>,
    input: LmsUniversityInput,
) -> Result<LmsUniversityDto, CommandError> {
    logged(
        "add_university",
        async {
            let base = probe_url(&state, &input.base_url, input.platform).await?;
            insert_validated(&state, &input.name, input.platform, &base.base_url)
        }
        .await,
    )
}

#[tauri::command]
pub async fn rename_university(
    state: State<'_, AppState>,
    id: UniversityId,
    name: String,
) -> Result<LmsUniversityDto, CommandError> {
    logged("rename_university", rename(&state, id, &name))
}

#[tauri::command]
pub async fn remove_university(
    state: State<'_, AppState>,
    id: UniversityId,
) -> Result<(), CommandError> {
    logged("remove_university", remove(&state, id))
}

#[tauri::command]
pub async fn lms_sign_in(
    app: AppHandle,
    state: State<'_, AppState>,
    id: UniversityId,
) -> Result<LmsUniversityDto, CommandError> {
    logged(
        "lms_sign_in",
        async {
            let university = find(&state, id)?;
            let captured = capture_credential(&app, &university, LoginMode::Interactive).await;
            complete_sign_in(&state, id, captured).await
        }
        .await,
    )
}

#[tauri::command]
pub async fn lms_sign_out(
    state: State<'_, AppState>,
    id: UniversityId,
) -> Result<LmsUniversityDto, CommandError> {
    logged("lms_sign_out", sign_out(&state, id))
}

#[tauri::command]
pub async fn lms_check_session(
    state: State<'_, AppState>,
    id: UniversityId,
) -> Result<LmsUniversityDto, CommandError> {
    logged("lms_check_session", check_session(&state, id).await)
}

fn find(state: &AppState, id: UniversityId) -> Result<LmsUniversity, CommandError> {
    state
        .universities
        .get_university(id)?
        .ok_or_else(|| CommandError::not_found(NOT_FOUND))
}

fn dto(state: &AppState, university: &LmsUniversity) -> Result<LmsUniversityDto, CommandError> {
    Ok(LmsUniversityDto::new(
        university,
        &state.courses.list_courses(university.id)?,
    ))
}

fn universities(state: &AppState) -> Result<Vec<LmsUniversityDto>, CommandError> {
    state
        .universities
        .list_universities()?
        .iter()
        .map(|u| dto(state, u))
        .collect()
}

async fn probe_url(
    state: &AppState,
    url: &str,
    platform: LmsPlatform,
) -> Result<ProbeDto, CommandError> {
    let base_url = normalize_base_url(url).map_err(|e| CommandError::validation(e.to_string()))?;
    probe(&state.http, platform, &base_url)
        .await
        .map_err(|error| match error {
            ProbeError::Mismatch(message) => CommandError::validation(message),
            ProbeError::Connection(_) => {
                CommandError::from(LmsError::Connection(error.to_string()))
            }
        })?;
    Ok(ProbeDto { base_url })
}

/// Validates and inserts without probing (the command probes first).
fn insert_validated(
    state: &AppState,
    name: &str,
    platform: LmsPlatform,
    url: &str,
) -> Result<LmsUniversityDto, CommandError> {
    let name = validation::name(name)?;
    let base_url = normalize_base_url(url).map_err(|e| CommandError::validation(e.to_string()))?;
    if state
        .universities
        .list_universities()?
        .iter()
        .any(|u| u.base_url == base_url)
    {
        return Err(CommandError::conflict(DUPLICATE_PLATFORM));
    }
    let university =
        state
            .universities
            .insert_university(&name, platform, &base_url, state.clock.now())?;
    dto(state, &university)
}

fn rename(
    state: &AppState,
    id: UniversityId,
    name: &str,
) -> Result<LmsUniversityDto, CommandError> {
    let name = validation::name(name)?;
    let university = state.universities.rename_university(id, &name)?;
    dto(state, &university)
}

/// Deletes the secret first so a failure never leaves a credential without its university.
fn remove(state: &AppState, id: UniversityId) -> Result<(), CommandError> {
    let university = find(state, id)?;
    state
        .secrets
        .delete(&lms_credential_key(university.platform, id))?;
    Ok(state.universities.delete_university(id)?)
}

async fn complete_sign_in(
    state: &AppState,
    id: UniversityId,
    captured: Result<String, LmsError>,
) -> Result<LmsUniversityDto, CommandError> {
    let university = find(state, id)?;
    let credential = captured?;
    match adopt_credential(
        &university,
        &credential,
        state.lms_factory.as_ref(),
        state.secrets.as_ref(),
    )
    .await
    {
        Ok(identity) => {
            let updated =
                state
                    .universities
                    .set_login(id, LoginState::Ok, Some(&identity.display_name))?;
            dto(state, &updated)
        }
        Err(error) => {
            if rejects_credential(&error) {
                state.universities.set_login(id, LoginState::Error, None)?;
            }
            Err(error.into())
        }
    }
}

/// Only a credential the platform rejected or could not use marks the university
/// `Error`; transient connection and local storage failures leave its state alone.
fn rejects_credential(error: &SessionError) -> bool {
    matches!(
        error,
        SessionError::Lms(LmsError::Auth(_) | LmsError::Protocol(_))
    )
}

fn sign_out(state: &AppState, id: UniversityId) -> Result<LmsUniversityDto, CommandError> {
    let university = find(state, id)?;
    state
        .secrets
        .delete(&lms_credential_key(university.platform, id))?;
    let updated = state.universities.set_login(id, LoginState::Never, None)?;
    dto(state, &updated)
}

async fn check_session(
    state: &AppState,
    id: UniversityId,
) -> Result<LmsUniversityDto, CommandError> {
    let university = find(state, id)?;
    let updated = ensure_session(
        &university,
        state.lms_factory.as_ref(),
        state.lms_authenticator.as_ref(),
        state.universities.as_ref(),
    )
    .await?;
    dto(state, &updated)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::CommandErrorKind;
    use crate::test_support::{harness, harness_with_lms, FakeLmsBehavior};
    use academic_core::ports::SecretStore;

    fn add(h: &crate::test_support::Harness, name: &str, url: &str) -> LmsUniversityDto {
        insert_validated(&h.state, name, LmsPlatform::Moodle, url).unwrap()
    }

    #[test]
    fn names_are_required_and_urls_normalized() {
        let h = harness();
        assert_eq!(
            insert_validated(&h.state, "  ", LmsPlatform::Moodle, "aulavirtual.uv.es")
                .unwrap_err()
                .message,
            "Enter a name"
        );
        let error = insert_validated(
            &h.state,
            "UV",
            LmsPlatform::Moodle,
            "http://aulavirtual.uv.es",
        )
        .unwrap_err();
        assert_eq!(error.kind, CommandErrorKind::Validation);
        let u = add(&h, " UV ", "https://aulavirtual.uv.es/my/");
        assert_eq!(
            (u.name.as_str(), u.base_url.as_str()),
            ("UV", "https://aulavirtual.uv.es")
        );
        assert_eq!(u.login_state, LoginState::Never);
    }

    #[test]
    fn the_same_platform_cannot_be_added_twice() {
        let h = harness();
        add(&h, "UV", "https://aulavirtual.uv.es");
        let error = insert_validated(
            &h.state,
            "Valencia",
            LmsPlatform::Moodle,
            "aulavirtual.uv.es/",
        )
        .unwrap_err();
        assert_eq!(error.kind, CommandErrorKind::Conflict);
        assert_eq!(error.message, "This platform is already added");
        assert_eq!(universities(&h.state).unwrap().len(), 1);
    }

    #[tokio::test]
    async fn successful_sign_in_stores_the_credential_and_marks_ok() {
        let h = harness();
        let u = add(&h, "UV", "https://aulavirtual.uv.es");
        let dto = complete_sign_in(&h.state, u.id, Ok("tok".into()))
            .await
            .unwrap();
        assert_eq!(dto.login_state, LoginState::Ok);
        assert_eq!(dto.account_label.as_deref(), Some("Ana Pérez"));
        assert_eq!(
            h.secrets
                .get(&lms_credential_key(LmsPlatform::Moodle, u.id))
                .unwrap()
                .as_deref(),
            Some("tok")
        );
    }

    #[tokio::test]
    async fn cancelled_sign_in_changes_nothing() {
        let h = harness();
        let u = add(&h, "UV", "https://aulavirtual.uv.es");
        let error = complete_sign_in(
            &h.state,
            u.id,
            Err(LmsError::Auth("Sign-in cancelled".into())),
        )
        .await
        .unwrap_err();
        assert_eq!(
            (error.kind, error.message.as_str()),
            (CommandErrorKind::Auth, "Sign-in cancelled")
        );
        assert_eq!(
            universities(&h.state).unwrap()[0].login_state,
            LoginState::Never
        );
    }

    #[tokio::test]
    async fn rejected_credential_marks_error_and_is_not_stored() {
        let h = harness_with_lms(FakeLmsBehavior {
            verify: || Err(LmsError::Auth("rejected".into())),
            ..FakeLmsBehavior::default()
        });
        let u = add(&h, "UV", "https://aulavirtual.uv.es");
        assert!(complete_sign_in(&h.state, u.id, Ok("tok".into()))
            .await
            .is_err());
        assert_eq!(
            universities(&h.state).unwrap()[0].login_state,
            LoginState::Error
        );
        assert_eq!(
            h.secrets
                .get(&lms_credential_key(LmsPlatform::Moodle, u.id))
                .unwrap(),
            None
        );
    }

    #[tokio::test]
    async fn connection_failure_during_sign_in_keeps_the_login_state_and_label() {
        let h = harness_with_lms(FakeLmsBehavior {
            verify: || Err(LmsError::Connection("down".into())),
            ..FakeLmsBehavior::default()
        });
        let u = add(&h, "UV", "https://aulavirtual.uv.es");
        h.state
            .universities
            .set_login(u.id, LoginState::Ok, Some("Ana Pérez"))
            .unwrap();
        let error = complete_sign_in(&h.state, u.id, Ok("tok".into()))
            .await
            .unwrap_err();
        assert_eq!(error.kind, CommandErrorKind::Connection);
        let after = &universities(&h.state).unwrap()[0];
        assert_eq!(after.login_state, LoginState::Ok);
        assert_eq!(after.account_label.as_deref(), Some("Ana Pérez"));
        assert_eq!(
            h.secrets
                .get(&lms_credential_key(LmsPlatform::Moodle, u.id))
                .unwrap(),
            None
        );
    }

    #[tokio::test]
    async fn check_session_without_secret_tries_renewal_then_reports_login_required() {
        let h = harness();
        let u = add(&h, "UV", "https://aulavirtual.uv.es");
        h.state
            .universities
            .set_login(u.id, LoginState::Ok, Some("Ana Pérez"))
            .unwrap();
        let error = check_session(&h.state, u.id).await.unwrap_err();
        assert_eq!(error.kind, CommandErrorKind::Auth);
        assert_eq!(
            universities(&h.state).unwrap()[0].login_state,
            LoginState::Expired
        );
    }

    #[test]
    fn sign_out_and_remove_delete_the_secret() {
        let h = harness();
        let u = add(&h, "UV", "https://aulavirtual.uv.es");
        let key = lms_credential_key(LmsPlatform::Moodle, u.id);
        h.secrets.set(&key, "tok").unwrap();
        let dto = sign_out(&h.state, u.id).unwrap();
        assert_eq!(dto.login_state, LoginState::Never);
        assert_eq!(h.secrets.get(&key).unwrap(), None);

        h.secrets.set(&key, "tok").unwrap();
        remove(&h.state, u.id).unwrap();
        assert_eq!(h.secrets.get(&key).unwrap(), None);
        assert!(universities(&h.state).unwrap().is_empty());
    }

    #[test]
    fn missing_university_is_not_found() {
        let h = harness();
        assert_eq!(
            sign_out(&h.state, 99).unwrap_err().kind,
            CommandErrorKind::NotFound
        );
        assert_eq!(
            rename(&h.state, 99, "x").unwrap_err().kind,
            CommandErrorKind::NotFound
        );
    }
}
