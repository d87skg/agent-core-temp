#[cfg(test)]
mod linearizability_tests {
    use anyhow::Result;
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    use tokio::time::{sleep, Duration};

    use crate::idempotency::manager::UltimateIdempotencyManager;

    #[tokio::test]
    async fn test_linearizable_execution() -> Result<()> {
        let manager = Arc::new(UltimateIdempotencyManager::new());
        let counter = Arc::new(AtomicUsize::new(0));

        let mut tasks = Vec::new();
        for i in 0..15 {
            let manager = manager.clone();
            let counter = counter.clone();

            tasks.push(tokio::spawn(async move {
                sleep(Duration::from_millis(i * 3)).await;
                manager
                    .execute("linear", "step", "intent", || async {
                        counter.fetch_add(1, Ordering::SeqCst);
                        Ok::<_, anyhow::Error>(99)
                    })
                    .await
            }));
        }

        for t in tasks {
            t.await??;
        }

        assert_eq!(counter.load(Ordering::SeqCst), 1);
        Ok(())
    }
}