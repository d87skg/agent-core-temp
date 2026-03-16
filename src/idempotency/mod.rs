//! # Idempotency
//! Provides Exactly-Once execution guarantees via CRDT and Fencing Tokens.
//! Supports in-memory and Sled backends.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::time::{Duration, SystemTime};
use crate::error::Result;

/// The status of a task record.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RecordStatus {
    /// The task is being processed (locked).
    Pending,
    /// The task completed successfully.
    Completed,
    /// The task failed.
    Failed,
}

/// A record stored in the idempotency backend.
///
/// Each record corresponds to a unique task identified by `key`.
/// The `version` field is used for optimistic concurrency control (CAS).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Record {
    /// Unique task identifier.
    pub key: String,
    /// Current status.
    pub status: RecordStatus,
    /// Result data (if completed).
    pub result: Option<Vec<u8>>,
    /// Version number for optimistic concurrency control.
    pub version: u64,
    /// Creation timestamp (milliseconds since epoch).
    pub created_at: u64,
    /// Last update timestamp.
    pub updated_at: u64,
    /// Expiration time (if any).
    pub expires_at: Option<u64>,
    /// Owner identifier (e.g., worker ID).
    pub owner: Option<String>,
}

impl Record {
    /// Creates a new pending record with the given key and TTL.
    pub fn new(key: &str, owner: Option<String>, ttl: Option<Duration>) -> Self {
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        Self {
            key: key.to_string(),
            status: RecordStatus::Pending,
            result: None,
            version: 1,
            created_at: now,
            updated_at: now,
            expires_at: ttl.map(|d| now + d.as_millis() as u64),
            owner,
        }
    }
}

/// Idempotency backend trait.
///
/// All methods are asynchronous and must be `Send + Sync` to be used across threads.
/// Implementations must ensure atomicity and linearizability for the `create`, `complete`, and `fail` operations.
#[async_trait]
pub trait IdempotencyBackend: Send + Sync + 'static {
    /// Attempts to create a new record.
    ///
    /// Returns `Ok(())` if the record did not exist and was created successfully.
    /// Returns `Err(AgentError::Conflict)` if a record with the same key already exists.
    async fn create(&self, key: &str, owner: Option<&str>, ttl: Option<Duration>) -> Result<Record>;

    /// Marks a task as completed and stores its result.
    ///
    /// # Arguments
    /// * `key` – The task identifier.
    /// * `result` – The result data (e.g., JSON bytes).
    /// * `expected_version` – The version the caller expects; used for CAS.
    ///
    /// Returns `Err(AgentError::VersionMismatch)` if the version does not match.
    async fn complete(&self, key: &str, result: Vec<u8>, expected_version: u64) -> Result<()>;

    /// Marks a task as failed with an error message.
    ///
    /// Similar to `complete`, but stores an error message instead of a result.
    async fn fail(&self, key: &str, error_msg: &str, expected_version: u64) -> Result<()>;

    /// Retrieves a record, if it exists.
    async fn get(&self, key: &str) -> Result<Option<Record>>;

    /// Deletes a record.
    async fn delete(&self, key: &str) -> Result<()>;

    /// Purges expired records.
    ///
    /// Returns the number of records removed.
    async fn purge_expired(&self) -> Result<usize>;

    /// Releases locks that have timed out.
    ///
    /// Records that have been in `Pending` state for longer than `timeout` are removed.
    async fn release_timed_out_locks(&self, timeout: Duration) -> Result<usize>;
}

#[cfg(feature = "sled-storage")]
pub mod sled;

pub mod memory;