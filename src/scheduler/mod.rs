//! # Scheduler
//!
//! Task queue with distributed support (memory or Redis). Implements retry, dead-letter, and priority.
//!
//! The core trait [`Scheduler`] defines operations for submitting, popping, acknowledging, and failing tasks.
//! Two implementations are provided:
//! - [`memory::MemoryScheduler`] – in-memory queue (for testing / single-node).
//! - [`redis::RedisScheduler`] – distributed queue backed by Redis Streams (requires `redis-scheduler` feature).

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::time::{Duration, SystemTime};
use uuid::Uuid;

use crate::error::Result;

/// A task to be scheduled.
///
/// Each task is identified by a unique `id`. Tasks can be delayed, have a priority,
/// and will be retried automatically up to `max_retries` times.
#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct Task {
    /// Unique task ID (UUID v4).
    pub id: String,
    /// Task type (e.g., "transfer", "exec").
    pub task_type: String,
    /// Payload data (usually JSON).
    pub payload: Vec<u8>,
    /// Priority level (0-10, 10 highest).
    pub priority: u8,
    /// Unix timestamp (milliseconds) when the task becomes eligible.
    pub delay_until: Option<u64>,
    /// Number of retries already performed.
    pub retry_count: u32,
    /// Maximum allowed retries (default 3).
    pub max_retries: u32,
    /// Creation timestamp (milliseconds since epoch).
    pub created_at: u64,
    /// Last error message (if any) from a failed execution.
    pub last_error: Option<String>,
}

impl Task {
    /// Creates a new task with the given parameters.
    ///
    /// # Arguments
    /// * `task_type` – A string describing the task category.
    /// * `payload` – Arbitrary data (usually serialized JSON).
    /// * `priority` – Priority value between 0 and 10.
    /// * `delay` – Optional delay before the task becomes eligible.
    ///
    /// The task ID is automatically generated as a UUID v4.
    /// `retry_count` starts at 0, `max_retries` defaults to 3.
    pub fn new(task_type: &str, payload: Vec<u8>, priority: u8, delay: Option<Duration>) -> Self {
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        Self {
            id: Uuid::new_v4().to_string(),
            task_type: task_type.to_string(),
            payload,
            priority,
            delay_until: delay.map(|d| now + d.as_millis() as u64),
            retry_count: 0,
            max_retries: 3,
            created_at: now,
            last_error: None,
        }
    }
}

/// Scheduler trait.
///
/// Implementations can be in-memory (for testing) or distributed (Redis).
/// All methods are asynchronous and must be `Send + Sync` to be used across threads.
#[async_trait]
pub trait Scheduler: Send + Sync + 'static {
    /// Submits a task to the queue.
    ///
    /// Returns the task ID (which is the same as `task.id`).
    async fn submit(&self, task: Task) -> Result<String>;

    /// Pops the next eligible task (non‑blocking).
    ///
    /// Returns `Ok(Some(task))` if a task is available, `Ok(None)` otherwise.
    async fn pop(&self) -> Result<Option<Task>>;

    /// Acknowledges that a task has been completed successfully.
    ///
    /// The task is removed from the queue permanently.
    async fn ack(&self, task_id: &str) -> Result<()>;

    /// Negative acknowledgment: the task failed and should be retried or moved to the dead‑letter queue.
    async fn nack(&self, task_id: &str, error: &str) -> Result<()>;

    /// Returns the number of tasks in the dead‑letter queue.
    async fn dead_letter_count(&self) -> Result<usize>;

    /// Purges expired tasks (optional).
    async fn purge_expired(&self) -> Result<usize>;
}

/// In‑memory scheduler (for testing and development).
pub mod memory;

/// Redis‑backed scheduler (requires `redis-scheduler` feature).
#[cfg(feature = "redis-scheduler")]
pub mod redis;