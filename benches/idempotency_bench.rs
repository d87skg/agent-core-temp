use criterion::{criterion_group, criterion_main, Criterion, BenchmarkId};
use agent_core_temp::idempotency::memory::MemoryBackend;
use agent_core_temp::idempotency::sled::SledBackend;
use agent_core_temp::idempotency::IdempotencyBackend;
use tokio::runtime::Runtime;
use tempfile::tempdir;

fn bench_memory_create(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let backend = MemoryBackend::new(1000);

    c.bench_function("memory_create", |b| {
        b.iter(|| {
            rt.block_on(async {
                let key = uuid::Uuid::new_v4().to_string();
                let _ = backend.create(&key, None, None).await;
            });
        })
    });
}

fn bench_memory_complete(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let backend = MemoryBackend::new(1000);
    let key = "bench_key";
    rt.block_on(async {
        backend.create(key, None, None).await.unwrap();
    });

    c.bench_function("memory_complete", |b| {
        b.iter(|| {
            rt.block_on(async {
                let _ = backend.complete(key, b"result".to_vec(), 1).await;
            });
        })
    });
}

fn bench_sled_create(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let dir = tempdir().unwrap();
    let backend = SledBackend::new(dir.path().to_str().unwrap(), 1000).unwrap();

    c.bench_function("sled_create", |b| {
        b.iter(|| {
            rt.block_on(async {
                let key = uuid::Uuid::new_v4().to_string();
                let _ = backend.create(&key, None, None).await;
            });
        })
    });
}

fn bench_sled_complete(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let dir = tempdir().unwrap();
    let backend = SledBackend::new(dir.path().to_str().unwrap(), 1000).unwrap();
    let key = "bench_key";
    rt.block_on(async {
        backend.create(key, None, None).await.unwrap();
    });

    c.bench_function("sled_complete", |b| {
        b.iter(|| {
            rt.block_on(async {
                let _ = backend.complete(key, b"result".to_vec(), 1).await;
            });
        })
    });
}

criterion_group!(benches, bench_memory_create, bench_memory_complete, bench_sled_create, bench_sled_complete);
criterion_main!(benches);