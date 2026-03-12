use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use tokio::time::Instant;
use tokio::task::JoinSet;

use agent_core_temp::idempotency::manager::UltimateIdempotencyManager;

const TOTAL_REQUESTS: usize = 1_000_000;
const WORKERS: usize = 64;

#[tokio::main]
async fn main() {
    println!("Starting load test");

    let manager = Arc::new(UltimateIdempotencyManager::new());
    let counter = Arc::new(AtomicUsize::new(0));

    let start = Instant::now();
    let mut joinset = JoinSet::new();

    for worker_id in 0..WORKERS {
        let manager = manager.clone();
        let counter = counter.clone();

        joinset.spawn(async move {
            for i in 0..(TOTAL_REQUESTS / WORKERS) {
                let _ = manager
                    .execute("load_test", &format!("step{}", i % 10), "intent", || async {
                        counter.fetch_add(1, Ordering::SeqCst);
                        Ok::<_, anyhow::Error>(1)
                    })
                    .await
                    .unwrap();
            }
            println!("Worker {} finished", worker_id);
        });
    }

    while let Some(res) = joinset.join_next().await {
        res.unwrap();
    }

    let duration = start.elapsed();

    println!("============================");
    println!("Load Test Finished");
    println!("Total Requests: {}", TOTAL_REQUESTS);
    println!("Actual Executions: {}", counter.load(Ordering::SeqCst));
    println!("Duration: {:?}", duration);

    let rps = TOTAL_REQUESTS as f64 / duration.as_secs_f64();
    println!("Throughput: {:.2} req/sec", rps);
}