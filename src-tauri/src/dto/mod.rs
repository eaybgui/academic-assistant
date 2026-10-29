//! Data transfer objects exchanged with the frontend (spec §10.1).
//!
//! JSON shape mirrors `src/lib/api/types.ts` exactly: camelCase fields, snake_case
//! enum strings, `YYYY-MM-DD` dates, `HH:MM` times and RFC 3339 timestamps. Output
//! DTOs are built from domain types; input DTOs are validated by
//! [`crate::validation`] before they reach the domain.

mod accounts;
mod catalog;
mod lms;
mod results;
mod runs;
mod settings;

pub use accounts::*;
pub use catalog::*;
pub use lms::*;
pub use results::*;
pub use runs::*;
pub use settings::*;
