//! `academic-infra`: adapters implementing `academic-core`'s ports — SQLite
//! persistence, the Windows Credential Manager secret store, IMAP and Microsoft
//! Graph email providers, and the jev/Ollama structured-model adapters.

pub mod db;
pub mod factory;
pub mod graph;
pub mod imap;
pub mod jev;
pub mod ollama;
pub mod secrets;
