//! LMS adapters (spec §5): the public platform probe, Moodle and Blackboard
//! clients, and the factory that builds them from stored credentials.

pub mod blackboard;
pub mod factory;
pub mod moodle;
pub mod probe;

use std::time::Duration;

use academic_core::ports::LmsError;

use crate::http_util::describe_transport_error;

/// Per-request timeout for every LMS call.
pub(crate) const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

pub(crate) fn connection_error(error: &reqwest::Error) -> LmsError {
    LmsError::Connection(format!(
        "Could not reach the platform: {}",
        describe_transport_error(error)
    ))
}
