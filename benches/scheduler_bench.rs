use criterion::{criterion_group, criterion_main, Criterion};
use agent_core_temp::scheduler::memory::MemoryScheduler;
#[cfg(feature = "redis-scheduler")]
use agent_core_temp::scheduler::redis::RedisScheduler;
use agent_core_temp::scheduler::{Scheduler, Task};
use tokio::runtime::Runtime;

fn bench_memory_submit(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let scheduler = MemoryScheduler::new();

    c.bench_function("memory_submit", |b| {
        b.iter(|| {
            rt.block_on(async {
                let task = Task::new("bench", b"payload".to_vec(), 5, None);
                let _ = scheduler.submit(task).await;
            });
        })
    });
}

fn bench_memory_pop_ack(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let scheduler = MemoryScheduler::new();

    c.bench_function("memory_pop_ack", |b| {
        b.iter(|| {
            rt.block_on(async {
                let task = Task::new("bench", b"payload".to_vec(), 5, None);
                let task_id = scheduler.submit(task).await.unwrap();
                let popped = scheduler.pop().await.unwrap().unwrap();
                assert_eq!(popped.id, task_id);
                scheduler.ack(&task_id).await.unwrap();
            });
        })
    });
}

#[cfg(feature = "redis-scheduler")]
fn bench_redis_submit(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let scheduler = rt.block_on(async {
        RedisScheduler::new(
            "redis://127.0.0.1:6379",
            "bench-stream",
            "bench-group",
            "bench-worker",
            3,
        )
        .await
        .unwrap()
    });

    // 预热阶段可忽略，直接开始测量
    c.bench_function("redis_submit", |b| {
        b.iter(|| {
            rt.block_on(async {
                let task = Task::new("bench", b"payload".to_vec(), 5, None);
                let _ = scheduler.submit(task).await;
            });
        })
    });
}

#[cfg(feature = "redis-scheduler")]
fn bench_redis_pop_ack(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let scheduler = rt.block_on(async {
        RedisScheduler::new(
            "redis://127.0.0.1:6379",
            "bench-stream",
            "bench-group",
            "bench-worker",
            3,
        )
        .await
        .unwrap()
    });

    // 预先提交一批任务，确保队列中有足够任务供测量
    for _ in 0..100 {
        let task = Task::new("bench", b"payload".to_vec(), 5, None);
        let _ = rt.block_on(async { scheduler.submit(task).await.unwrap() });
    }

    c.bench_function("redis_pop_ack", |b| {
        b.iter(|| {
            rt.block_on(async {
                let popped = scheduler.pop().await.unwrap().unwrap();
                scheduler.ack(&popped.id).await.unwrap();
            });
        })
    });
}

criterion_group!(memory_benches, bench_memory_submit, bench_memory_pop_ack);

#[cfg(feature = "redis-scheduler")]
criterion_group!(redis_benches, bench_redis_submit, bench_redis_pop_ack);

#[cfg(feature = "redis-scheduler")]
criterion_main!(memory_benches, redis_benches);

#[cfg(not(feature = "redis-scheduler"))]
criterion_main!(memory_benches);