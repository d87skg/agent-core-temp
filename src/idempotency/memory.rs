// src/idempotency/memory.rs
use super::{IdempotencyBackend, Record, RecordStatus, Result};
use async_trait::async_trait;
use dashmap::DashMap;
use lru::LruCache;
use std::num::NonZeroUsize;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::sync::Mutex;

pub struct MemoryBackend {
    records: Arc<DashMap<String, Record>>,
    cache: Arc<Mutex<LruCache<String, Vec<u8>>>>, // 结果缓存
}

impl MemoryBackend {
    pub fn new(cache_size: usize) -> Self {
        Self {
            records: Arc::new(DashMap::new()),
            cache: Arc::new(Mutex::new(LruCache::new(
                NonZeroUsize::new(cache_size).unwrap(),
            ))),
        }
    }

    fn now_millis() -> u64 {
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64
    }
}

#[async_trait]
impl IdempotencyBackend for MemoryBackend {
    async fn create(&self, key: &str, owner: Option<&str>, ttl: Option<Duration>) -> Result<Record> {
        let mut record = Record::new(key, owner.map(|s| s.to_string()), ttl);
        // DashMap 的 entry 提供原子插入
        use dashmap::mapref::entry::Entry;
        match self.records.entry(key.to_string()) {
            Entry::Occupied(_) => Err(crate::error::AgentError::Conflict(key.to_string())),
            Entry::Vacant(v) => {
                record.version = 1;
                v.insert(record.clone());
                Ok(record)
            }
        }
    }

    async fn complete(&self, key: &str, result: Vec<u8>, expected_version: u64) -> Result<()> {
        let mut entry = self
            .records
            .get_mut(key)
            .ok_or_else(|| crate::error::AgentError::NotFound(key.to_string()))?;
        if entry.version != expected_version {
            return Err(crate::error::AgentError::VersionMismatch {
                expected: expected_version,
                actual: entry.version,
            });
        }
        entry.status = RecordStatus::Completed;
        entry.result = Some(result.clone());
        entry.version += 1;
        entry.updated_at = Self::now_millis();
        // 更新缓存
        let mut cache = self.cache.lock().await;
        cache.put(key.to_string(), result);
        Ok(())
    }

    async fn fail(&self, key: &str, error_msg: &str, expected_version: u64) -> Result<()> {
        let mut entry = self
            .records
            .get_mut(key)
            .ok_or_else(|| crate::error::AgentError::NotFound(key.to_string()))?;
        if entry.version != expected_version {
            return Err(crate::error::AgentError::VersionMismatch {
                expected: expected_version,
                actual: entry.version,
            });
        }
        entry.status = RecordStatus::Failed;
        entry.result = Some(error_msg.as_bytes().to_vec());
        entry.version += 1;
        entry.updated_at = Self::now_millis();
        // 从缓存移除
        let mut cache = self.cache.lock().await;
        cache.pop(key);
        Ok(())
    }

    async fn get(&self, key: &str) -> Result<Option<Record>> {
        Ok(self.records.get(key).map(|r| r.clone()))
    }

    async fn delete(&self, key: &str) -> Result<()> {
        self.records.remove(key);
        let mut cache = self.cache.lock().await;
        cache.pop(key);
        Ok(())
    }

    async fn purge_expired(&self) -> Result<usize> {
        let now = Self::now_millis();
        let keys_to_remove: Vec<String> = self
            .records
            .iter()
            .filter(|r| {
                if let Some(exp) = r.expires_at {
                    exp <= now
                } else {
                    false
                }
            })
            .map(|r| r.key.clone())
            .collect();
        for key in &keys_to_remove {
            self.records.remove(key);
            let mut cache = self.cache.lock().await;
            cache.pop(key);
        }
        Ok(keys_to_remove.len())
    }

    async fn release_timed_out_locks(&self, timeout: Duration) -> Result<usize> {
        let now = Self::now_millis();
        let threshold = now - timeout.as_millis() as u64;
        let keys_to_release: Vec<String> = self
            .records
            .iter()
            .filter(|r| {
                r.status == RecordStatus::Pending && r.updated_at <= threshold
            })
            .map(|r| r.key.clone())
            .collect();
        for key in &keys_to_release {
            // 直接删除（可改为标记为可重试，这里简单删除）
            self.records.remove(key);
            let mut cache = self.cache.lock().await;
            cache.pop(key);
        }
        Ok(keys_to_release.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_create_complete() {
        let backend = MemoryBackend::new(100);
        let key = "test1";
        let record = backend.create(key, None, None).await.unwrap();
        assert_eq!(record.status, RecordStatus::Pending);
        assert_eq!(record.version, 1);

        let result = b"done".to_vec();
        backend.complete(key, result.clone(), 1).await.unwrap();

        let rec = backend.get(key).await.unwrap().unwrap();
        assert_eq!(rec.status, RecordStatus::Completed);
        assert_eq!(rec.result, Some(result));
        assert_eq!(rec.version, 2);
    }

    #[tokio::test]
    async fn test_version_conflict() {
        let backend = MemoryBackend::new(100);
        let key = "test2";
        backend.create(key, None, None).await.unwrap();
        // 用错误的版本号 complete
        let err = backend.complete(key, b"x".to_vec(), 2).await.unwrap_err();
        assert!(matches!(err, crate::error::AgentError::VersionMismatch { .. }));
    }

    #[tokio::test]
    async fn test_concurrent_create() {
        let backend = std::sync::Arc::new(MemoryBackend::new(100));
        let key = "concurrent";
        let mut handles = vec![];
        for _ in 0..10 {
            let b = backend.clone();
            let k = key.to_string();
            handles.push(tokio::spawn(async move {
                b.create(&k, None, None).await
            }));
        }
        let results: Vec<_> = futures::future::join_all(handles)
            .await
            .into_iter()
            .map(|r| r.unwrap())
            .collect();
        let success_count = results.iter().filter(|r| r.is_ok()).count();
        assert_eq!(success_count, 1); // 只有一个成功
    }
}