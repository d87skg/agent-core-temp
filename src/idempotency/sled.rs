// src/idempotency/sled.rs
use super::{IdempotencyBackend, Record, RecordStatus, Result};
use async_trait::async_trait;
use serde_json;
use sled::Db;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::sync::RwLock;

pub struct SledBackend {
    db: Arc<Db>,
    cache: Arc<RwLock<lru::LruCache<String, Record>>>,
}

impl SledBackend {
    pub fn new(path: &str, cache_size: usize) -> Result<Self> {
        let db = sled::open(path)?;
        Ok(Self {
            db: Arc::new(db),
            cache: Arc::new(RwLock::new(lru::LruCache::new(
                std::num::NonZeroUsize::new(cache_size).unwrap(),
            ))),
        })
    }

    fn now_millis() -> u64 {
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64
    }
}

#[async_trait]
impl IdempotencyBackend for SledBackend {
    async fn create(&self, key: &str, owner: Option<&str>, ttl: Option<Duration>) -> Result<Record> {
        if self.db.contains_key(key)? {
            return Err(crate::error::AgentError::Conflict(key.to_string()));
        }

        let record = Record::new(key, owner.map(|s| s.to_string()), ttl);
        let value = serde_json::to_vec(&record)?;
        self.db.insert(key, value)?;
        Ok(record)
    }

    async fn complete(&self, key: &str, result: Vec<u8>, expected_version: u64) -> Result<()> {
        let existing = self.db.get(key)?
            .ok_or_else(|| crate::error::AgentError::NotFound(key.to_string()))?;
        let mut record: Record = serde_json::from_slice(&existing)?;

        if record.version != expected_version {
            return Err(crate::error::AgentError::VersionMismatch {
                expected: expected_version,
                actual: record.version,
            });
        }

        record.status = RecordStatus::Completed;
        record.result = Some(result);
        record.version += 1;
        record.updated_at = Self::now_millis();

        let new_value = serde_json::to_vec(&record)?;
        self.db.insert(key, new_value)?;

        let mut cache = self.cache.write().await;
        cache.put(key.to_string(), record);
        Ok(())
    }

    async fn fail(&self, key: &str, error_msg: &str, expected_version: u64) -> Result<()> {
        let existing = self.db.get(key)?
            .ok_or_else(|| crate::error::AgentError::NotFound(key.to_string()))?;
        let mut record: Record = serde_json::from_slice(&existing)?;

        if record.version != expected_version {
            return Err(crate::error::AgentError::VersionMismatch {
                expected: expected_version,
                actual: record.version,
            });
        }

        record.status = RecordStatus::Failed;
        record.result = Some(error_msg.as_bytes().to_vec());
        record.version += 1;
        record.updated_at = Self::now_millis();

        let new_value = serde_json::to_vec(&record)?;
        self.db.insert(key, new_value)?;

        let mut cache = self.cache.write().await;
        cache.pop(key);
        Ok(())
    }

    async fn get(&self, key: &str) -> Result<Option<Record>> {
        // 先查缓存（使用 peek 避免修改顺序）
        {
            let cache = self.cache.read().await;
            if let Some(record) = cache.peek(key) {
                return Ok(Some(record.clone()));
            }
        }

        if let Some(data) = self.db.get(key)? {
            let record: Record = serde_json::from_slice(&data)?;
            let mut cache = self.cache.write().await;
            cache.put(key.to_string(), record.clone());
            Ok(Some(record))
        } else {
            Ok(None)
        }
    }

    async fn delete(&self, key: &str) -> Result<()> {
        self.db.remove(key)?;
        let mut cache = self.cache.write().await;
        cache.pop(key);
        Ok(())
    }

    async fn purge_expired(&self) -> Result<usize> {
        let now = Self::now_millis();
        let mut to_remove = Vec::new();

        for item in self.db.iter() {
            let (key, value) = item?;
            let record: Record = serde_json::from_slice(&value)?;
            if let Some(exp) = record.expires_at {
                if exp <= now {
                    to_remove.push(key.to_vec());
                }
            }
        }

        for key in &to_remove {
            self.db.remove(key.as_slice())?;
        }

        Ok(to_remove.len())
    }

    async fn release_timed_out_locks(&self, timeout: Duration) -> Result<usize> {
        let now = Self::now_millis();
        let threshold = now - timeout.as_millis() as u64;
        let mut to_remove = Vec::new();

        for item in self.db.iter() {
            let (key, value) = item?;
            let record: Record = serde_json::from_slice(&value)?;
            if record.status == RecordStatus::Pending && record.updated_at <= threshold {
                to_remove.push(key.to_vec());
            }
        }

        for key in &to_remove {
            self.db.remove(key.as_slice())?;
        }

        Ok(to_remove.len())
    }
}