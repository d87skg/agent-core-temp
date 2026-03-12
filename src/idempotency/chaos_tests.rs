#[cfg(test)]
mod chaos_tests {
    use anyhow::Result;
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    use tokio::time::{sleep, Duration};

    use crate::idempotency::manager::UltimateIdempotencyManager;
    use crate::idempotency::keygen::IdempotencyKeyGenerator;

    #[tokio::test]
    async fn chaos_worker_crash() -> Result<()> {
        let manager = Arc::new(UltimateIdempotencyManager::new());
        let counter = Arc::new(AtomicUsize::new(0));

        let manager_clone = manager.clone();
        let counter_clone = counter.clone();

        // First worker crashes mid execution
        let _ = tokio::spawn(async move {
            let _ = manager_clone
                .execute("chaos", "crash", "intent", || async {
                    counter_clone.fetch_add(1, Ordering::SeqCst);
                    sleep(Duration::from_millis(50)).await;
                    panic!("simulated worker crash");
                })
                .await;
        });

        sleep(Duration::from_millis(20)).await;

        // second worker should recover and succeed
        let result = manager
            .execute("chaos", "crash", "intent", || async {
                counter.fetch_add(1, Ordering::SeqCst);
                Ok::<_, anyhow::Error>(100)
            })
            .await?;

        assert_eq!(result, 100);
        Ok(())
    }

    #[tokio::test]
    async fn chaos_network_delay() -> Result<()> {
        let manager = Arc::new(UltimateIdempotencyManager::new());

        let result = manager
            .execute("chaos", "delay", "intent", || async {
                sleep(Duration::from_millis(200)).await;
                Ok::<_, anyhow::Error>(55)
            })
            .await?;

        assert_eq!(result, 55);
        Ok(())
    }

    #[tokio::test]
    async fn chaos_high_contention() -> Result<()> {
        let manager = Arc::new(UltimateIdempotencyManager::new());
        let counter = Arc::new(AtomicUsize::new(0));

        let mut tasks = Vec::new();
        for _ in 0..50 {
            let manager = manager.clone();
            let counter = counter.clone();

            tasks.push(tokio::spawn(async move {
                manager
                    .execute("chaos", "contention", "intent", || async {
                        counter.fetch_add(1, Ordering::SeqCst);
                        sleep(Duration::from_millis(10)).await;
                        Ok::<_, anyhow::Error>(7)
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