// src/idempotency/rocksdb.rs
use super::{IdempotencyBackend, Record, RecordStatus, Result};
use async_trait::async_trait;
use rocksdb::{Options, DB, WriteBatch, IteratorMode};
use serde_json;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::sync::RwLock;

pub struct RocksDbBackend {
    db: Arc<DB>,
    cache: Arc<RwLock<lru::LruCache<String, Record>>>,
}

impl RocksDbBackend {
    pub fn new(path: &str, cache_size: usize) -> Result<Self> {
        let mut opts = Options::default();
        opts.create_if_missing(true);
        opts.set_compression_type(rocksdb::DBCompressionType::Lz4);
        let db = DB::open(&opts, path)?;

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
impl IdempotencyBackend for RocksDbBackend {
    async fn create(&self, key: &str, owner: Option<&str>, ttl: Option<Duration>) -> Result<Record> {
        let key_bytes = key.as_bytes();
        if self.db.get(key_bytes)?.is_some() {
            return Err(crate::error::AgentError::Conflict(key.to_string()));
        }

        let record = Record::new(key, owner.map(|s| s.to_string()), ttl);
        let value = serde_json::to_vec(&record)?;
        self.db.put(key_bytes, value)?;
        Ok(record)
    }

    async fn complete(&self, key: &str, result: Vec<u8>, expected_version: u64) -> Result<()> {
        let key_bytes = key.as_bytes();
        let existing = self.db.get(key_bytes)?
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
        self.db.put(key_bytes, new_value)?;

        // 更新缓存
        let mut cache = self.cache.write().await;
        cache.put(key.to_string(), record);
        Ok(())
    }

    async fn fail(&self, key: &str, error_msg: &str, expected_version: u64) -> Result<()> {
        let key_bytes = key.as_bytes();
        let existing = self.db.get(key_bytes)?
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
        self.db.put(key_bytes, new_value)?;

        let mut cache = self.cache.write().await;
        cache.pop(key);
        Ok(())
    }

    async fn get(&self, key: &str) -> Result<Option<Record>> {
        // 先查缓存
        {
            let cache = self.cache.read().await;
            if let Some(record) = cache.get(key) {
                return Ok(Some(record.clone()));
            }
        }

        let key_bytes = key.as_bytes();
        if let Some(data) = self.db.get(key_bytes)? {
            let record: Record = serde_json::from_slice(&data)?;
            let mut cache = self.cache.write().await;
            cache.put(key.to_string(), record.clone());
            Ok(Some(record))
        } else {
            Ok(None)
        }
    }

    async fn delete(&self, key: &str) -> Result<()> {
        self.db.delete(key.as_bytes())?;
        let mut cache = self.cache.write().await;
        cache.pop(key);
        Ok(())
    }

    async fn purge_expired(&self) -> Result<usize> {
        let now = Self::now_millis();
        let mut batch = WriteBatch::default();
        let mut count = 0;

        let iter = self.db.iterator(IteratorMode::Start);
        for item in iter {
            let (key, value) = item?;
            let record: Record = serde_json::from_slice(&value)?;
            if let Some(exp) = record.expires_at {
                if exp <= now {
                    batch.delete(key);
                    count += 1;
                }
            }
        }

        if count > 0 {
            self.db.write(batch)?;
        }
        Ok(count)
    }

    async fn release_timed_out_locks(&self, timeout: Duration) -> Result<usize> {
        let now = Self::now_millis();
        let threshold = now - timeout.as_millis() as u64;
        let mut batch = WriteBatch::default();
        let mut count = 0;

        let iter = self.db.iterator(IteratorMode::Start);
        for item in iter {
            let (key, value) = item?;
            let record: Record = serde_json::from_slice(&value)?;
            if record.status == RecordStatus::Pending && record.updated_at <= threshold {
                batch.delete(key);
                count += 1;
            }
        }

        if count > 0 {
            self.db.write(batch)?;
        }
        Ok(count)
    }
}