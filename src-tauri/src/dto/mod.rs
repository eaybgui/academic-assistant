//! Data transfer objects exchanged with the frontend (spec §10.1).
//!
//! JSON shape mirrors `src/lib/api/types.ts` exactly: camelCase fields, snake_case
//! enum strings, `YYYY-MM-DD` dates, `HH:MM` times and RFC 3339 timestamps. Output
//! DTOs are built from domain types; input DTOs are validated by
//! [`crate::validation`] before they reach the domain.

#[cfg(not(target_os = "android"))]
mod accounts;
#[cfg(not(target_os = "android"))]
mod catalog;
mod lms;
#[cfg(not(target_os = "android"))]
mod results;
#[cfg(not(target_os = "android"))]
mod runs;
#[cfg(not(target_os = "android"))]
mod settings;

#[cfg(not(target_os = "android"))]
pub use accounts::*;
#[cfg(not(target_os = "android"))]
pub use catalog::*;
pub use lms::*;
#[cfg(not(target_os = "android"))]
pub use results::*;
#[cfg(not(target_os = "android"))]
pub use runs::*;
#[cfg(not(target_os = "android"))]
pub use settings::*;
