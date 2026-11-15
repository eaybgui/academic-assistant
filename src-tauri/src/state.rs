//! `AppState`: the composition of repositories, adapters and the check runner
//! shared by every command through Tauri's managed state.

use std::sync::Arc;

use academic_core::lms::{LmsRepositories, LmsSyncRunner};
#[cfg(not(target_os = "android"))]
use academic_core::pipeline::{CheckRunner, Repositories};
#[cfg(not(target_os = "android"))]
use academic_core::ports::{
    AccountRepository, CatalogRepository, EmailProviderFactory, ModelProvider, ResultRepository,
    RunObserver, RunRepository, SettingsRepository, SyncStateRepository,
};
use academic_core::ports::{
    Clock, CourseRepository, LmsAuthenticator, LmsClientFactory, LmsItemRepository,
    LmsSyncObserver, PersonalTaskRepository, SecretStore, UniversityRepository,
};
use academic_infra::db::Database;
#[cfg(not(target_os = "android"))]
use chrono::NaiveDate;
use tracing_appender::non_blocking::WorkerGuard;

/// Adapters and services `AppState` is composed from. `Database` is passed
/// separately because every repository port is implemented by it.
pub struct Collaborators {
    pub secrets: Arc<dyn SecretStore>,
    #[cfg(not(target_os = "android"))]
    pub factory: Arc<dyn EmailProviderFactory>,
    #[cfg(not(target_os = "android"))]
    pub models: Arc<dyn ModelProvider>,
    pub http: reqwest::Client,
    #[cfg(not(target_os = "android"))]
    pub observer: Arc<dyn RunObserver>,
    pub clock: Arc<dyn Clock>,
    pub lms_factory: Arc<dyn LmsClientFactory>,
    pub lms_authenticator: Arc<dyn LmsAuthenticator>,
    pub lms_observer: Arc<dyn LmsSyncObserver>,
}

pub struct AppState {
    #[cfg(not(target_os = "android"))]
    pub accounts: Arc<dyn AccountRepository>,
    #[cfg(not(target_os = "android"))]
    pub catalog: Arc<dyn CatalogRepository>,
    #[cfg(not(target_os = "android"))]
    pub sync: Arc<dyn SyncStateRepository>,
    pub universities: Arc<dyn UniversityRepository>,
    pub courses: Arc<dyn CourseRepository>,
    pub items: Arc<dyn LmsItemRepository>,
    pub personal_tasks: Arc<dyn PersonalTaskRepository>,
    #[cfg(not(target_os = "android"))]
    pub results: Arc<dyn ResultRepository>,
    #[cfg(not(target_os = "android"))]
    pub runs: Arc<dyn RunRepository>,
    #[cfg(not(target_os = "android"))]
    pub settings: Arc<dyn SettingsRepository>,
    pub secrets: Arc<dyn SecretStore>,
    #[cfg(not(target_os = "android"))]
    pub factory: Arc<dyn EmailProviderFactory>,
    #[cfg(not(target_os = "android"))]
    pub models: Arc<dyn ModelProvider>,
    pub http: reqwest::Client,
    pub clock: Arc<dyn Clock>,
    #[cfg(not(target_os = "android"))]
    pub observer: Arc<dyn RunObserver>,
    pub lms_factory: Arc<dyn LmsClientFactory>,
    pub lms_authenticator: Arc<dyn LmsAuthenticator>,
    pub lms_observer: Arc<dyn LmsSyncObserver>,
    #[cfg(not(target_os = "android"))]
    pub runner: Arc<CheckRunner>,
    pub lms_sync: Arc<LmsSyncRunner>,
    /// Keeps the non-blocking log writer flushing for the lifetime of the app.
    _log_guard: Option<WorkerGuard>,
}

impl AppState {
    /// Wires the single `db` into every repository port and builds the
    /// [`CheckRunner`] and [`LmsSyncRunner`] over the same adapters.
    pub fn new(db: Arc<Database>, collaborators: Collaborators) -> Self {
        let Collaborators {
            secrets,
            #[cfg(not(target_os = "android"))]
            factory,
            #[cfg(not(target_os = "android"))]
            models,
            http,
            #[cfg(not(target_os = "android"))]
            observer,
            clock,
            lms_factory,
            lms_authenticator,
            lms_observer,
        } = collaborators;
        #[cfg(not(target_os = "android"))]
        let runner = Arc::new(CheckRunner::new(
            Repositories {
                accounts: db.clone(),
                catalog: db.clone(),
                sync: db.clone(),
                processed: db.clone(),
                results: db.clone(),
                runs: db.clone(),
                settings: db.clone(),
            },
            factory.clone(),
            models.clone(),
            observer.clone(),
            clock.clone(),
        ));
        let lms_sync = Arc::new(LmsSyncRunner::new(
            LmsRepositories {
                universities: db.clone(),
                courses: db.clone(),
                items: db.clone(),
            },
            lms_factory.clone(),
            lms_authenticator.clone(),
            lms_observer.clone(),
            clock.clone(),
        ));
        Self {
            #[cfg(not(target_os = "android"))]
            accounts: db.clone(),
            #[cfg(not(target_os = "android"))]
            catalog: db.clone(),
            #[cfg(not(target_os = "android"))]
            sync: db.clone(),
            universities: db.clone(),
            courses: db.clone(),
            items: db.clone(),
            personal_tasks: db.clone(),
            #[cfg(not(target_os = "android"))]
            results: db.clone(),
            #[cfg(not(target_os = "android"))]
            runs: db.clone(),
            #[cfg(not(target_os = "android"))]
            settings: db,
            secrets,
            #[cfg(not(target_os = "android"))]
            factory,
            #[cfg(not(target_os = "android"))]
            models,
            http,
            clock,
            #[cfg(not(target_os = "android"))]
            observer,
            lms_factory,
            lms_authenticator,
            lms_observer,
            #[cfg(not(target_os = "android"))]
            runner,
            lms_sync,
            _log_guard: None,
        }
    }

    pub fn with_log_guard(mut self, guard: WorkerGuard) -> Self {
        self._log_guard = Some(guard);
        self
    }

    /// The current date in the user's local time zone.
    #[cfg(not(target_os = "android"))]
    pub fn today(&self) -> NaiveDate {
        self.clock
            .now()
            .with_timezone(&self.clock.local_offset())
            .date_naive()
    }
}
