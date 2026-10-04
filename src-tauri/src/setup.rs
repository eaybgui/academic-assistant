//! Application startup: data directory, logging, database and composition of
//! [`AppState`] (spec §12; see `docs/app/logging-and-data.md`).

use std::error::Error;
use std::path::Path;
use std::sync::Arc;

use academic_core::ports::{Clock, RunRepository, SecretStore, SystemClock};
use academic_infra::db::Database;
use academic_infra::factory::{http_client, DefaultEmailProviderFactory, DefaultModelProvider};
use academic_infra::lms::factory::DefaultLmsClientFactory;
use academic_infra::secrets::KeyringSecretStore;
use tauri::{App, Manager};
use tracing_appender::non_blocking::WorkerGuard;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{fmt, EnvFilter};

use crate::lms_auth::TauriLmsAuthenticator;
use crate::observer::TauriRunObserver;
use crate::state::{AppState, Collaborators};

const DATABASE_FILE: &str = "academic-assistant.db";
const LOGS_DIR: &str = "logs";
const LOG_FILE_PREFIX: &str = "academic-assistant.log";
const DEFAULT_LOG_FILTER: &str = "info";
/// Directives that keep protocol-level dependency logging (which can include
/// mail traffic) at `warn` in release builds, whatever `RUST_LOG` says.
const RELEASE_DEPENDENCY_DIRECTIVES: [&str; 5] = [
    "async_imap=warn",
    "hyper=warn",
    "reqwest=warn",
    "rustls=warn",
    "tokio_rustls=warn",
];

/// Resolves the app data directory, starts logging, opens the database, marks
/// runs interrupted by a previous crash as failed and manages the [`AppState`].
pub fn initialize(app: &App) -> Result<(), Box<dyn Error>> {
    let data_dir = app.path().app_data_dir()?;
    std::fs::create_dir_all(&data_dir)?;
    let log_guard = init_logging(&data_dir.join(LOGS_DIR))?;

    let state = build_state(app, &data_dir)?.with_log_guard(log_guard);
    app.manage(state);
    tracing::info!("application started");
    Ok(())
}

fn build_state(app: &App, data_dir: &Path) -> Result<AppState, Box<dyn Error>> {
    let db = Arc::new(Database::open(&data_dir.join(DATABASE_FILE))?);
    let clock: Arc<dyn Clock> = Arc::new(SystemClock);

    let interrupted = db.mark_interrupted(clock.now())?;
    if interrupted > 0 {
        tracing::warn!(interrupted, "marked interrupted runs as failed");
    }

    let http = http_client();
    let secrets: Arc<dyn SecretStore> = Arc::new(KeyringSecretStore);
    let factory = Arc::new(DefaultEmailProviderFactory::new(
        secrets.clone(),
        db.clone(),
        http.clone(),
    ));
    let models = Arc::new(DefaultModelProvider::new(secrets.clone(), http.clone()));
    let lms_factory = Arc::new(DefaultLmsClientFactory::new(secrets.clone(), http.clone()));
    let lms_authenticator = Arc::new(TauriLmsAuthenticator::new(
        app.handle().clone(),
        lms_factory.clone(),
        secrets.clone(),
    ));

    Ok(AppState::new(
        db,
        Collaborators {
            secrets,
            factory,
            models,
            http,
            observer: Arc::new(TauriRunObserver::new(app.handle().clone())),
            clock,
            lms_factory,
            lms_authenticator,
        },
    ))
}

/// Installs the global `tracing` subscriber: daily rolling file in `logs_dir`
/// (level from `RUST_LOG`, default `info`) plus stderr in debug builds. The
/// returned guard must stay alive so buffered lines are flushed.
fn init_logging(logs_dir: &Path) -> Result<WorkerGuard, Box<dyn Error>> {
    std::fs::create_dir_all(logs_dir)?;
    let appender = RollingFileAppender::builder()
        .rotation(Rotation::DAILY)
        .filename_prefix(LOG_FILE_PREFIX)
        .build(logs_dir)?;
    let (writer, guard) = tracing_appender::non_blocking(appender);

    let env_filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(DEFAULT_LOG_FILTER));
    let filter = cap_dependency_logging(env_filter, cfg!(not(debug_assertions)));
    let file_layer = fmt::layer().with_ansi(false).with_writer(writer);
    let stderr_layer = cfg!(debug_assertions).then(|| fmt::layer().with_writer(std::io::stderr));

    tracing_subscriber::registry()
        .with(filter)
        .with(file_layer)
        .with(stderr_layer)
        .try_init()?;
    Ok(guard)
}

/// In `release` builds, appends [`RELEASE_DEPENDENCY_DIRECTIVES`] so `RUST_LOG`
/// cannot raise the logging of the mail/HTTP protocol dependencies above `warn`.
/// Debug builds keep `filter` unchanged.
fn cap_dependency_logging(filter: EnvFilter, release: bool) -> EnvFilter {
    if !release {
        return filter;
    }
    RELEASE_DEPENDENCY_DIRECTIVES
        .into_iter()
        .fold(filter, |filter, directive| {
            filter.add_directive(directive.parse().expect("static directive is valid"))
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn release_builds_cap_dependency_logging_even_when_rust_log_raises_it() {
        let filter =
            cap_dependency_logging(EnvFilter::new("trace,async_imap=trace"), true).to_string();
        for directive in RELEASE_DEPENDENCY_DIRECTIVES {
            assert!(filter.contains(directive), "{directive} in {filter}");
        }
        assert!(!filter.contains("async_imap=trace"), "{filter}");
    }

    #[test]
    fn debug_builds_leave_the_filter_unchanged() {
        let filter = cap_dependency_logging(EnvFilter::new("async_imap=trace"), false).to_string();
        assert_eq!(filter, "async_imap=trace");
    }
}
