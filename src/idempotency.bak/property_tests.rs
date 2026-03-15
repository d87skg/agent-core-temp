#[cfg(test)]
mod property_tests {
    use proptest::prelude::*;
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    use tokio::runtime::Runtime;

    use crate::idempotency::manager::UltimateIdempotencyManager;

    proptest! {
        #![proptest_config = ProptestConfig::with_cases(10)]
        #[test]
        fn property_exactly_once(n in 1u8..20) {
            let rt = Runtime::new().unwrap();
            rt.block_on(async {
                let manager = Arc::new(UltimateIdempotencyManager::new());
                let counter = Arc::new(AtomicUsize::new(0));

                let mut tasks = Vec::new();
                for _ in 0..n {
                    let manager = manager.clone();
                    let counter = counter.clone();

                    tasks.push(tokio::spawn(async move {
                        let _ = manager
                            .execute("prop", "test", "intent", || async {
                                counter.fetch_add(1, Ordering::SeqCst);
                                Ok::<_, anyhow::Error>(1)
                            })
                            .await;
                    }));
                }

                for t in tasks {
                    let _ = t.await;
                }

                assert_eq!(counter.load(Ordering::SeqCst), 1);
            });
        }
    }
}