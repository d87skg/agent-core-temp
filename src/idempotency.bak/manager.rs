use crate::idempotency::keygen::IdempotencyKeyGenerator;
use crate::idempotency::predictor::IntentPredictor;
use crate::idempotency::store::CRDTStore;

pub struct UltimateIdempotencyManager {
    keygen: IdempotencyKeyGenerator,
    predictor: IntentPredictor,
    store: CRDTStore,
}

impl UltimateIdempotencyManager {
    pub fn new() -> Self {
        Self {
            keygen: IdempotencyKeyGenerator::new(),
            predictor: IntentPredictor::new(),
            store: CRDTStore::new(),
        }
    }

    pub async fn execute<F, Fut, T>(&self, workflow_id: &str, step_id: &str, intent: &str, f: F) -> Result<T, anyhow::Error>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<T, anyhow::Error>>,
        T: serde::Serialize + for<'de> serde::Deserialize<'de>,
    {
        let _risk = self.predictor.risk_score(intent);
        let key = self.keygen.generate(workflow_id, step_id);

        // 尝试创建记录（原子操作）
        if !self.store.try_create(&key).await? {
            // 记录已存在，但结果可能尚未写入 → 轮询等待（最多 500 次，每次 10ms，总 5 秒）
            for _attempt in 0..500 {
                if let Some(data) = self.store.get_result(&key).await? {
                    let result: T = serde_json::from_slice(&data)?;
                    return Ok(result);
                }
                tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
            }
            anyhow::bail!("timeout waiting for idempotent result after 5 seconds");
        }

        // 首次执行，执行业务逻辑
        let result = f().await?;
        let serialized = serde_json::to_vec(&result)?;
        self.store.complete(&key, serialized).await?;
        Ok(result)
    }
}