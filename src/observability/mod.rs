use metrics_exporter_prometheus::PrometheusBuilder;
use prometheus::{register_counter, register_gauge, register_histogram, Counter, Gauge, Histogram};
use std::net::SocketAddr;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

/// 初始化可观测性系统（Metrics + Tracing）
pub fn init_observability() -> anyhow::Result<()> {
    // 1. 初始化结构化日志（JSON格式）
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .json()
        .with_target(true)
        .with_thread_ids(true)
        .with_thread_names(true)
        .with_file(true)
        .with_line_number(true)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;
    info!("Tracing initialized with JSON format");

    // 2. 初始化 Prometheus 指标
    PrometheusBuilder::new()
        .with_http_listener(SocketAddr::from(([127, 0, 0, 1], 9091)))
        .install()?;
    info!("Prometheus metrics exporter started on http://127.0.0.1:9090");

    Ok(())
}

/// 自定义指标
pub mod metrics {
    use once_cell::sync::Lazy;
    use super::{Counter, Gauge, Histogram};
    use prometheus::{register_counter, register_gauge, register_histogram};

    pub static TASKS_TOTAL: Lazy<Counter> = Lazy::new(|| {
        register_counter!("tasks_total", "Total number of tasks executed").unwrap()
    });

    pub static TASKS_SUCCESS: Lazy<Counter> = Lazy::new(|| {
        register_counter!("tasks_success", "Total number of successful tasks").unwrap()
    });

    pub static TASKS_FAILED: Lazy<Counter> = Lazy::new(|| {
        register_counter!("tasks_failed", "Total number of failed tasks").unwrap()
    });

    pub static TASKS_RETRIED: Lazy<Counter> = Lazy::new(|| {
        register_counter!("tasks_retried", "Total number of retried tasks").unwrap()
    });

    pub static WORKER_ACTIVE: Lazy<Gauge> = Lazy::new(|| {
        register_gauge!("worker_active", "Number of active workers").unwrap()
    });

    pub static TASK_DURATION: Lazy<Histogram> = Lazy::new(|| {
        register_histogram!("task_duration_seconds", "Task execution duration").unwrap()
    });

    pub fn inc_tasks_total() { TASKS_TOTAL.inc(); }
    pub fn inc_tasks_success() { TASKS_SUCCESS.inc(); }
    pub fn inc_tasks_failed() { TASKS_FAILED.inc(); }
    pub fn inc_tasks_retried() { TASKS_RETRIED.inc(); }
    pub fn set_worker_active(count: f64) { WORKER_ACTIVE.set(count); }
    pub fn observe_task_duration(secs: f64) { TASK_DURATION.observe(secs); }
}