//! `academic-infra`: adapters implementing `academic-core`'s ports — SQLite
//! persistence, the Windows Credential Manager secret store, IMAP and Microsoft
//! Graph email providers, the jev/Ollama structured-model adapters, and the
//! LMS (Moodle/Blackboard) clients.

pub mod db;
pub mod factory;
#[cfg(not(target_os = "android"))]
pub mod graph;
mod http_util;
#[cfg(not(target_os = "android"))]
pub mod imap;
#[cfg(not(target_os = "android"))]
pub mod jev;
pub mod lms;
#[cfg(not(target_os = "android"))]
pub mod ollama;
pub mod secrets;
