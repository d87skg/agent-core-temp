// src/error.rs
use thiserror::Error;

#[derive(Error, Debug)]
pub enum AgentError {
    #[error("Storage error: {0}")]
    Storage(String),

    #[error("Conflict: key {0} already exists")]
    Conflict(String),

    #[error("Record not found: {0}")]
    NotFound(String),

    #[error("Version mismatch (expected {expected}, got {actual})")]
    VersionMismatch { expected: u64, actual: u64 },

    #[error("Encryption error: {0}")]
    Encryption(String),

    #[error("Queue error: {0}")]
    Queue(String),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Internal error: {0}")]
    Internal(#[from] anyhow::Error),
}

pub type Result<T> = std::result::Result<T, AgentError>;