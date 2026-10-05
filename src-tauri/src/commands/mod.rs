//! Tauri commands (spec §10.2), grouped by area. Each command is a thin async
//! wrapper: it calls a plain function that takes `&AppState` (unit-testable without
//! a running app) and logs failures through [`logged`].

pub mod accounts;
pub mod catalog;
pub mod results;
pub mod runs;
pub mod settings;
pub mod tracker;
pub mod universities;

use crate::error::CommandError;

/// Logs a failed command (kind, message and internal detail only) and passes the
/// result through unchanged.
fn logged<T>(command: &str, result: Result<T, CommandError>) -> Result<T, CommandError> {
    result.map_err(|error| error.logged(command))
}
