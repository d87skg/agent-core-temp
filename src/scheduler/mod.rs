// src/scheduler/mod.rs
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::time::{Duration, SystemTime};
use uuid::Uuid;

use crate::error::Result;

// 导出 memory 模块，使其始终可用
pub mod memory;

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct Task {
    pub id: String,
    pub task_type: String,
    pub payload: Vec<u8>,
    pub priority: u8,                 // 0-10，值越大优先级越高
    pub delay_until: Option<u64>,     // Unix 时间戳（毫秒）
    pub retry_count: u32,
    pub max_retries: u32,
    pub created_at: u64,
    pub last_error: Option<String>,
}

impl Task {
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

#[async_trait]
pub trait Scheduler: Send + Sync + 'static {
    async fn submit(&self, task: Task) -> Result<String>;
    async fn pop(&self) -> Result<Option<Task>>;
    async fn ack(&self, task_id: &str) -> Result<()>;
    async fn nack(&self, task_id: &str, error: &str) -> Result<()>;
    async fn dead_letter_count(&self) -> Result<usize>;
    async fn purge_expired(&self) -> Result<usize>;
}
// src/scheduler/mod.rs
// ... 原有代码不变

#[cfg(feature = "redis-scheduler")]
pub mod redis;