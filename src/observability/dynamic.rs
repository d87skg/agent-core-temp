// src/observability/dynamic.rs
use metrics::{Counter, Gauge, Histogram};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

pub struct DynamicMetrics {
    counters: Arc<RwLock<HashMap<String, Counter>>>,
    gauges: Arc<RwLock<HashMap<String, Gauge>>>,
    histograms: Arc<RwLock<HashMap<String, Histogram>>>,
}

impl DynamicMetrics {
    pub fn new() -> Self {
        Self {
            counters: Arc::new(RwLock::new(HashMap::new())),
            gauges: Arc::new(RwLock::new(HashMap::new())),
            histograms: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn get_counter(&self, name: &str) -> Counter {
        let mut counters = self.counters.write().unwrap();
        counters
            .entry(name.to_string())
            .or_insert_with(|| metrics::counter!(name.to_string()))
            .clone()
    }

    pub fn get_gauge(&self, name: &str) -> Gauge {
        let mut gauges = self.gauges.write().unwrap();
        gauges
            .entry(name.to_string())
            .or_insert_with(|| metrics::gauge!(name.to_string()))
            .clone()
    }

    pub fn get_histogram(&self, name: &str) -> Histogram {
        let mut histograms = self.histograms.write().unwrap();
        histograms
            .entry(name.to_string())
            .or_insert_with(|| metrics::histogram!(name.to_string()))
            .clone()
    }

    pub fn increment_counter(&self, name: &str, value: u64) {
        self.get_counter(name).increment(value);
    }

    pub fn set_gauge(&self, name: &str, value: f64) {
        self.get_gauge(name).set(value);
    }

    pub fn record_histogram(&self, name: &str, value: f64) {
        self.get_histogram(name).record(value);
    }
}