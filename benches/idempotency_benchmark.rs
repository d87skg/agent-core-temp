use criterion::{criterion_group, criterion_main, Criterion};
use std::sync::Arc;
use tokio::runtime::Runtime;
use tokio::time::{sleep, Duration};

use agent_core_temp::idempotency::manager::UltimateIdempotencyManager;

fn benchmark_idempotency_execute(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let manager = Arc::new(UltimateIdempotencyManager::new());

    c.bench_function("idempotency_execute_single", |b| {
        b.to_async(&rt).iter(|| async {
            let _ = manager
                .execute("bench_wf", "step", "intent", || async {
                    sleep(Duration::from_micros(50)).await;
                    Ok::<_, anyhow::Error>(42)
                })
                .await
                .unwrap();
        });
    });
}

fn benchmark_idempotency_concurrent(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let manager = Arc::new(UltimateIdempotencyManager::new());

    c.bench_function("idempotency_execute_concurrent", |b| {
        b.to_async(&rt).iter(|| async {
            let mut tasks = Vec::new();

            for _ in 0..10 {
                let manager = manager.clone();
                tasks.push(tokio::spawn(async move {
                    manager
                        .execute("bench_concurrent", "step", "intent", || async {
                            sleep(Duration::from_micros(50)).await;
                            Ok::<_, anyhow::Error>(1)
                        })
                        .await
                }));
            }

            for t in tasks {
                let _ = t.await.unwrap().unwrap();
            }
        });
    });
}

criterion_group!(
    benches,
    benchmark_idempotency_execute,
    benchmark_idempotency_concurrent
);
criterion_main!(benches);