use criterion::{criterion_group, criterion_main, Criterion, black_box};
use agent_core_temp::observability::metrics;

fn bench_metrics(c: &mut Criterion) {
    let mut group = c.benchmark_group("metrics");
    
    group.bench_function("inc_tasks_total", |b| {
        b.iter(|| black_box(metrics::inc_tasks_total()))
    });
    
    group.bench_function("inc_tasks_success", |b| {
        b.iter(|| black_box(metrics::inc_tasks_success()))
    });
    
    group.finish();
}

criterion_group!(benches, bench_metrics);
criterion_main!(benches);