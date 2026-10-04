//! `academic-core`: pure domain model, ports and business logic for the
//! Academic Assistant email analysis pipeline. Contains no I/O.

pub mod analyzer;
pub mod dates;
pub mod domain;
pub mod filters;
pub mod normalize;
pub mod pipeline;
pub mod ports;
