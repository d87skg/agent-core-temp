use anyhow::Result;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::time::Duration;
use crate::idempotency::manager::UltimateIdempotencyManager;

#[tokio::test]
async fn test_concurrent_create() -> Result<()> {
    let manager = Arc::new(UltimateIdempotencyManager::new());
    let counter = Arc::new(AtomicUsize::new(0));
    let mut handles = vec![];

    for i in 0..10 {
        let manager = manager.clone();
        let counter = counter.clone();
        handles.push(tokio::spawn(async move {
            manager
                .execute("concurrent_wf", "step_a", "intent", || async move {
                    counter.fetch_add(1, Ordering::SeqCst);
                    tokio::time::sleep(Duration::from_millis(50)).await;
                    Ok::<_, anyhow::Error>(i)
                })
                .await
        }));
    }

    let mut results = Vec::new();
    for handle in handles {
        results.push(handle.await?);
    }

    // 1. 所有任务都应成功返回（幂等性保证所有调用都返回结果）
    for res in &results {
        assert!(res.is_ok(), "任务应成功返回");
    }

    // 2. 所有成功结果必须相同（由第一个任务产生）
    let first_success = results[0].as_ref().unwrap().clone();
    for res in &results {
        assert_eq!(res.as_ref().unwrap(), &first_success, "所有结果应相同");
    }

    // 3. 验证闭包只执行一次
    assert_eq!(counter.load(Ordering::SeqCst), 1, "业务逻辑应只执行一次");

    Ok(())
}