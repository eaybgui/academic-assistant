//! Builds the LMS client for a university from its stored credential.

use std::sync::Arc;

use academic_core::domain::{lms_credential_key, LmsPlatform, LmsUniversity};
use academic_core::ports::{LmsClient, LmsClientFactory, LmsError, SecretStore};

use super::blackboard::{decode_session, BlackboardClient};
use super::moodle::MoodleClient;

pub struct DefaultLmsClientFactory {
    secrets: Arc<dyn SecretStore>,
    http: reqwest::Client,
}

impl DefaultLmsClientFactory {
    pub fn new(secrets: Arc<dyn SecretStore>, http: reqwest::Client) -> Self {
        Self { secrets, http }
    }
}

impl LmsClientFactory for DefaultLmsClientFactory {
    fn create(&self, university: &LmsUniversity) -> Result<Box<dyn LmsClient>, LmsError> {
        let credential = self
            .secrets
            .get(&lms_credential_key(university.platform, university.id))
            .map_err(|e| LmsError::Protocol(e.to_string()))?
            .ok_or_else(|| LmsError::Auth(format!("Sign in to {} to continue", university.name)))?;
        self.create_with_credential(university, &credential)
    }

    fn create_with_credential(
        &self,
        university: &LmsUniversity,
        credential: &str,
    ) -> Result<Box<dyn LmsClient>, LmsError> {
        let base_url = university.base_url.clone();
        Ok(match university.platform {
            LmsPlatform::Moodle => Box::new(MoodleClient::new(
                self.http.clone(),
                base_url,
                credential.to_string(),
            )),
            LmsPlatform::Blackboard => Box::new(BlackboardClient::new(
                self.http.clone(),
                base_url,
                &decode_session(credential)?,
            )),
        })
    }
}
