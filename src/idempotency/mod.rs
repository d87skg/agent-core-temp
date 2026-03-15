// src/idempotency/mod.rs
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::time::{Duration, SystemTime};

pub use crate::error::Result;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RecordStatus {
    Pending,    // 正在处理（锁）
    Completed,  // 成功完成
    Failed,     // 失败
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Record {
    pub key: String,
    pub status: RecordStatus,
    pub result: Option<Vec<u8>>,
    pub version: u64,               // 用于 CAS
    pub created_at: u64,             // Unix 时间戳（毫秒）
    pub updated_at: u64,
    pub expires_at: Option<u64>,     // 过期时间（毫秒）
    pub owner: Option<String>,        // 持有锁的 worker 标识
}

impl Record {
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

// ==================== Trait 定义 ====================
#[async_trait]
pub trait IdempotencyBackend: Send + Sync + 'static {
    /// 创建记录（原子锁）
    async fn create(&self, key: &str, owner: Option<&str>, ttl: Option<Duration>) -> Result<Record>;

    /// 完成记录（CAS 基于版本号）
    async fn complete(&self, key: &str, result: Vec<u8>, expected_version: u64) -> Result<()>;

    /// 标记失败（CAS 基于版本号）
    async fn fail(&self, key: &str, error_msg: &str, expected_version: u64) -> Result<()>;

    /// 获取记录
    async fn get(&self, key: &str) -> Result<Option<Record>>;

    /// 删除记录
    async fn delete(&self, key: &str) -> Result<()>;

    /// 清理过期记录
    async fn purge_expired(&self) -> Result<usize>;

    /// 释放超时锁（将 Pending 状态超过 ttl 的记录删除或标记为可重试）
    async fn release_timed_out_locks(&self, timeout: Duration) -> Result<usize>;
}
pub mod memory;
#[cfg(feature = "sled-storage")]
pub mod sled;