// src/observability/mod.rs
use metrics::{describe_counter, describe_gauge, describe_histogram};
use metrics_exporter_prometheus::{PrometheusBuilder, PrometheusHandle};
use std::sync::OnceLock;
use anyhow::Result;

static PROMETHEUS_HANDLE: OnceLock<Result<PrometheusHandle, anyhow::Error>> = OnceLock::new();

#[macro_export]
macro_rules! metrics {
    (counter, $name:literal, $value:expr) => {
        ::metrics::counter!($name).increment($value as u64);
    };
    (gauge, $name:literal, $value:expr) => {
        ::metrics::gauge!($name).set($value as f64);
    };
    (histogram, $name:literal, $value:expr) => {
        ::metrics::histogram!($name).record($value as f64);
    };
    (counter, $name:literal, $value:expr, $($k:literal => $v:expr),* $(,)?) => {
        ::metrics::counter!($name $(, $k => $v.to_string())*).increment($value as u64);
    };
    (gauge, $name:literal, $value:expr, $($k:literal => $v:expr),* $(,)?) => {
        ::metrics::gauge!($name $(, $k => $v.to_string())*).set($value as f64);
    };
    (histogram, $name:literal, $value:expr, $($k:literal => $v:expr),* $(,)?) => {
        ::metrics::histogram!($name $(, $k => $v.to_string())*).record($value as f64);
    };
}

pub fn register_static_metrics() {
    describe_counter!("agent.tasks_total", "Total tasks processed");
    describe_counter!("agent.tasks_completed", "Tasks completed");
    describe_counter!("agent.tasks_failed", "Tasks failed");
    describe_gauge!("agent.memory_usage_mb", "Memory usage in MB");
    describe_gauge!("agent.cpu_usage_seconds", "CPU usage");
    describe_histogram!("agent.request_duration_ms", "Request duration");
    describe_counter!("agent.redis_ops_total", "Redis operations");
    describe_counter!("agent.wasm.plugins_loaded", "WASM plugins loaded");
    describe_counter!("agent.wasm.fuel_consumed", "WASM fuel consumed");
    describe_histogram!("agent.wasm_exec_duration_ms", "WASM execution duration");
}

/// 初始化 Prometheus recorder，但不启动 HTTP 服务器
pub fn init_observability() -> Result<()> {
    let result = PROMETHEUS_HANDLE.get_or_init(|| {
        let builder = PrometheusBuilder::new();
        builder
            .install_recorder()
            .map_err(|e| anyhow::anyhow!("Prometheus install failed: {:?}", e))
    });
    match result {
        Ok(_) => {
            register_static_metrics();
            println!("✅ Prometheus recorder initialized");
            Ok(())
        }
        Err(e) => Err(anyhow::anyhow!("{}", e)),
    }
}

/// 获取 Prometheus 句柄，用于生成 metrics 响应
pub fn prometheus_handle() -> Option<&'static PrometheusHandle> {
    PROMETHEUS_HANDLE.get().and_then(|res| res.as_ref().ok())
}

// ====================== 便捷函数 ======================
pub fn inc_requests(n: u64) {
    metrics!(counter, "agent.requests_total", n);
}

pub fn record_wasm_duration(duration_ms: f64, plugin_name: &str, version: &str) {
    metrics!(histogram, "agent.wasm_exec_duration_ms", duration_ms,
        "plugin" => plugin_name,
        "version" => version);
}

pub fn record_wasm_fuel(fuel: u64, plugin_name: &str) {
    metrics!(counter, "agent.wasm.fuel_consumed", fuel, "plugin" => plugin_name);
}

pub fn record_wasm_plugin_loaded(name: &str, version: &str) {
    metrics!(counter, "agent.wasm.plugins_loaded", 1,
        "plugin_name" => name,
        "plugin_version" => version);
}

pub fn record_wasm_error(error_type: &str, plugin_name: &str) {
    metrics!(counter, "agent.wasm.errors", 1,
        "error_type" => error_type,
        "plugin_name" => plugin_name);
}

pub fn increment_tasks_total() {
    metrics!(counter, "agent.tasks_total", 1);
}

pub fn set_tasks_running(count: u64) {
    metrics!(gauge, "agent.tasks_running", count as f64);
}

pub fn increment_tasks_completed() {
    metrics!(counter, "agent.tasks_completed", 1);
}

pub fn increment_tasks_failed() {
    metrics!(counter, "agent.tasks_failed", 1);
}

pub fn record_request_duration(seconds: f64) {
    metrics!(histogram, "agent.request_duration_seconds", seconds);
}