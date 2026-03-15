use criterion::{criterion_group, criterion_main, Criterion, black_box};
use agent_core_temp::observability;

fn bench_metrics(c: &mut Criterion) {
    let mut group = c.benchmark_group("metrics");
    
    group.bench_function("increment_tasks_total", |b| {
        b.iter(|| black_box(observability::increment_tasks_total()))
    });
    
    group.bench_function("increment_tasks_completed", |b| {
        b.iter(|| black_box(observability::increment_tasks_completed()))
    });
    
    group.finish();
}

criterion_group!(benches, bench_metrics);
criterion_main!(benches);