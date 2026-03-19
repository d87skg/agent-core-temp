use metrics::{counter, gauge, histogram};
use once_cell::sync::Lazy;

pub static REDIS_TASKS_SUBMITTED: Lazy<Counter> = Lazy::new(|| {
    counter!("redis_tasks_submitted_total", "number of tasks submitted to Redis")
});

pub static REDIS_TASKS_POPPED: Lazy<Counter> = Lazy::new(|| {
    counter!("redis_tasks_popped_total", "number of tasks popped from Redis")
});

pub static REDIS_TASKS_ACKED: Lazy<Counter> = Lazy::new(|| {
    counter!("redis_tasks_acked_total", "number of tasks acked")
});

pub static REDIS_TASKS_NACKED: Lazy<Counter> = Lazy::new(|| {
    counter!("redis_tasks_nacked_total", "number of tasks nacked")
});

pub static REDIS_DEAD_LETTER_SIZE: Lazy<Gauge> = Lazy::new(|| {
    gauge!("redis_dead_letter_size", "current size of dead letter queue")
});

pub static REDIS_OPERATION_FAILURES: Lazy<Counter> = Lazy::new(|| {
    counter!("redis_operation_failures_total", "number of Redis operation failures")
});