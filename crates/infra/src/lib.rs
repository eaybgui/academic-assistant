//! `academic-infra`: adapters implementing `academic-core`'s ports — SQLite
//! persistence, the Windows Credential Manager secret store, IMAP and Microsoft
//! Graph email providers, the jev/Ollama structured-model adapters, and the
//! LMS (Moodle/Blackboard) clients.

pub mod db;
pub mod factory;
pub mod graph;
mod http_util;
pub mod imap;
pub mod jev;
pub mod lms;
pub mod ollama;
pub mod secrets;
