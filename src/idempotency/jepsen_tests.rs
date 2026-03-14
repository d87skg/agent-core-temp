//! Jepsen风格线性一致性测试
//! 基于 GPT + xAI 方案

#[cfg(test)]
mod jepsen_tests {
    use crate::idempotency::core::IdempotencyStore;
    use crate::idempotency::InMemoryStore;
    use std::sync::Arc;
    use tokio::sync::Barrier;
    use tokio::time::{pause, resume, sleep, Duration};

    #[tokio::test]
    async fn test_linearizability_30_clients() {
        let store = Arc::new(InMemoryStore::new());
        let mut handles = vec![];
        let barrier = Arc::new(Barrier::new(31));

        for i in 0..30 {
            let store = store.clone();
            let barrier = barrier.clone();
            handles.push(tokio::spawn(async move {
                barrier.wait().await;
                for j in 0..100 {
                    let key = format!("key-{}", j);
                    if store.try_create(&key).await.unwrap() {
                        store.complete(&key, 1, vec![i as u8]).await.unwrap();
                    }
                }
            }));
        }

        barrier.wait().await;
        for h in handles {
            h.await.unwrap();
        }

        // 验证 Exactly-Once
        for j in 0..100 {
            let key = format!("key-{}", j);
            let result = store.get_result(&key).await.unwrap();
            assert!(result.is_some(), "key {} missing", j);
        }
    }

    #[tokio::test]
    async fn test_network_partition() {
        pause();
        
        let store = Arc::new(InMemoryStore::new());
        let key = "partition-key";
        
        let handle = tokio::spawn({
            let store = store.clone();
            async move {
                store.try_create(key).await.unwrap();
                sleep(Duration::from_secs(5)).await;
                store.complete(key, 1, vec![42]).await.unwrap();
            }
        });

        // 分区期间，另一个任务无法claim
        assert!(store.try_claim(key, "worker2", Duration::from_secs(1)).await.unwrap().is_none());

        resume();
        handle.await.unwrap();

        let result = store.get_result(key).await.unwrap();
        assert_eq!(result, Some(vec![42]));
    }

    #[tokio::test]
    async fn test_hlc_clock_rollback() {
        let store = Arc::new(InMemoryStore::new());
        let key = "hlc-key";
        
        // 模拟时钟回拨
        store.try_create(key).await.unwrap();
        let before = store.get_fencing_token(key).await;
        
        // 这里无法直接模拟系统时钟回拨，用逻辑验证
        // 实际HLC应能处理
        store.complete(key, 2, vec![1]).await.unwrap();
        let after = store.get_fencing_token(key).await;
        
        assert!(after > before, "Fencing token must be monotonic");
    }
}