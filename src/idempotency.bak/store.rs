use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdempotencyRecord {
    pub key: String,
    pub status: IdempotencyStatus,
    pub result: Option<Vec<u8>>,
    pub version: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum IdempotencyStatus {
    Pending,
    Processing,
    Completed,
    Failed,
}

pub struct CRDTStore {
    inner: Arc<RwLock<HashMap<String, IdempotencyRecord>>>,
}

impl CRDTStore {
    pub fn new() -> Self {
        Self { inner: Arc::new(RwLock::new(HashMap::new())) }
    }

    pub async fn try_create(&self, key: &str) -> Result<bool, anyhow::Error> {
        let mut map = self.inner.write().await;
        if map.contains_key(key) {
            Ok(false)
        } else {
            map.insert(key.to_string(), IdempotencyRecord {
                key: key.to_string(),
                status: IdempotencyStatus::Processing,
                result: None,
                version: 1,
            });
            Ok(true)
        }
    }

    pub async fn complete(&self, key: &str, result: Vec<u8>) -> Result<(), anyhow::Error> {
        let mut map = self.inner.write().await;
        if let Some(rec) = map.get_mut(key) {
            rec.status = IdempotencyStatus::Completed;
            rec.result = Some(result);
            rec.version += 1;
        }
        Ok(())
    }

    pub async fn get_result(&self, key: &str) -> Result<Option<Vec<u8>>, anyhow::Error> {
        let map = self.inner.read().await;
        Ok(map.get(key).and_then(|r| r.result.clone()))
    }
}