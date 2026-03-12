use anyhow::Result;
use std::sync::{
    Arc,
    Mutex,
    atomic::{AtomicUsize, Ordering},
};
use tokio::time::{sleep, Duration};

use agent_core_temp::idempotency::manager::UltimateIdempotencyManager;

#[derive(Debug, Clone)]
struct Operation {
    client_id: usize,
    op_type: OpType,
    value: Option<u64>,
    timestamp: u128,
}

#[derive(Debug, Clone)]
enum OpType {
    Invoke,
    Ok,
}

fn check_linearizable(history: &[Operation]) {
    let mut seen_value = None;
    for op in history {
        if let OpType::Ok = op.op_type {
            match seen_value {
                None => seen_value = op.value,
                Some(v) => {
                    assert_eq!(
                        Some(v),
                        op.value,
                        "Linearizability violation detected"
                    );
                }
            }
        }
    }
}

#[tokio::test]
async fn jepsen_exactly_once_test() -> Result<()> {
    let manager = Arc::new(UltimateIdempotencyManager::new());
    let counter = Arc::new(AtomicUsize::new(0));
    let history: Arc<Mutex<Vec<Operation>>> = Arc::new(Mutex::new(Vec::new()));

    let mut tasks = Vec::new();
    for client_id in 0..30 {
        let manager = manager.clone();
        let counter = counter.clone();
        let history = history.clone();

        tasks.push(tokio::spawn(async move {
            let ts = std::time::Instant::now();
            history.lock().unwrap().push(Operation {
                client_id,
                op_type: OpType::Invoke,
                value: None,
                timestamp: ts.elapsed().as_millis(),
            });

            let result = manager
                .execute("jepsen", "step", "intent", || async {
                    counter.fetch_add(1, Ordering::SeqCst);
                    sleep(Duration::from_millis(10)).await;
                    Ok::<_, anyhow::Error>(42u64)
                })
                .await
                .unwrap();

            history.lock().unwrap().push(Operation {
                client_id,
                op_type: OpType::Ok,
                value: Some(result),
                timestamp: ts.elapsed().as_millis(),
            });
        }));
    }

    for t in tasks {
        t.await?;
    }

    let history = history.lock().unwrap();
    check_linearizable(&history);
    assert_eq!(
        counter.load(Ordering::SeqCst),
        1,
        "Exactly-once violation detected"
    );
    Ok(())
}