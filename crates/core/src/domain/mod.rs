//! Domain model: accounts, emails, AI analysis results, the course/university catalog,
//! check runs, stored results, settings and secret-store key helpers.
//!
//! Types here carry no I/O; persistence and provider adapters live in `academic-infra`
//! and implement the ports in [`crate::ports`].

mod account;
mod analysis;
mod catalog;
mod email;
mod lms;
mod results;
mod run;
mod secrets;
mod settings;

pub use account::*;
pub use analysis::*;
pub use catalog::*;
pub use email::*;
pub use lms::*;
pub use results::*;
pub use run::*;
pub use secrets::*;
pub use settings::*;

/// Error returned by every domain enum's `FromStr` implementation when the input
/// does not match one of its snake_case variant strings. Used when reading enum
/// columns back out of SQLite.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("invalid value: {0}")]
pub struct ParseEnumError(pub String);

/// Implements `as_str(&self) -> &'static str` and `FromStr` for a fieldless enum,
/// both driven by the same snake_case string table. Keeps the mapping in one place
/// instead of duplicating a `match` per direction for every domain enum.
macro_rules! enum_str {
    ($ty:ident { $($variant:ident => $str:expr),+ $(,)? }) => {
        impl $ty {
            pub fn as_str(&self) -> &'static str {
                match self {
                    $(Self::$variant => $str,)+
                }
            }
        }

        impl std::str::FromStr for $ty {
            type Err = $crate::domain::ParseEnumError;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                match s {
                    $($str => Ok(Self::$variant),)+
                    other => Err($crate::domain::ParseEnumError(other.to_string())),
                }
            }
        }
    };
}

pub(crate) use enum_str;
