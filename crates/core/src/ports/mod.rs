//! Ports: the traits and error types `academic-infra` adapters implement and
//! `academic-core` (and eventually `src-tauri`) depend on. No implementation lives
//! here, only contracts.

mod email;
mod model;
mod repositories;
mod runtime;
mod secrets;

pub use email::*;
pub use model::*;
pub use repositories::*;
pub use runtime::*;
pub use secrets::*;
