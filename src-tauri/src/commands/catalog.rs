//! Command: read-only catalog (universities and tracked courses) for the email screens.

use tauri::State;

use super::logged;
use crate::dto::UniversityDto;
use crate::error::CommandError;
use crate::state::AppState;

#[tauri::command]
pub async fn get_catalog(state: State<'_, AppState>) -> Result<Vec<UniversityDto>, CommandError> {
    logged("get_catalog", catalog(&state))
}

fn catalog(state: &AppState) -> Result<Vec<UniversityDto>, CommandError> {
    Ok(UniversityDto::from_catalog(&state.catalog.catalog()?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::harness;

    #[test]
    fn catalog_is_empty_without_universities() {
        let h = harness();
        assert!(catalog(&h.state).unwrap().is_empty());
    }
}
