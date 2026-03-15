use criterion::{criterion_group, criterion_main, Criterion};
use agent_core_temp::scheduler::RedisScheduler;
use tokio::runtime::Runtime;

fn bench_scheduler_submit(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let scheduler = rt.block_on(async {
        RedisScheduler::new("redis://127.0.0.1:6379").await.expect("failed to connect to Redis")
    });

    c.bench_function("scheduler_submit", |b| {
        b.to_async(&rt).iter(|| async {
            let _ = scheduler.submit("bench", b"hello".to_vec()).await;
        })
    });
}

criterion_group!(scheduler_benches, bench_scheduler_submit);
criterion_main!(scheduler_benches);