use agent_core_temp::observability;
use agent_core_temp::ingress;
use agent_core_temp::scheduler::memory::MemoryScheduler;
use agent_core_temp::scheduler::Scheduler;
#[cfg(feature = "redis-scheduler")]
use agent_core_temp::scheduler::redis::RedisScheduler;
use agent_core_temp::idempotency::IdempotencyBackend;
use agent_core_temp::idempotency::memory::MemoryBackend;
#[cfg(feature = "sled-storage")]
use agent_core_temp::idempotency::sled::SledBackend;
use agent_core_temp::workflow::ParallelExecutor;
use agent_core_temp::extension::Extension;
use anyhow::{anyhow, Result};
use std::env;
use std::sync::Arc;
use tracing_subscriber;
use rand::Rng;
use serde_json::Value;

// 模拟插件
struct MockExtension;

impl Extension for MockExtension {
    fn name(&self) -> &str {
        "mock-plugin"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn init(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    fn call(
        &self,
        method: &str,
        payload: Value,
    ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        println!("MockExtension called: method={}, payload={}", method, payload);
        // 模拟返回成功
        Ok(serde_json::json!({ "result": "success", "method": method }))
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("info")
        .with_target(true)
        .init();

    println!("🚀 Initializing observability...");
    observability::init_observability()?;
    println!("✅ Observability initialized");

    // 1. 选择调度器后端
    let scheduler_type = env::var("SCHEDULER_TYPE").unwrap_or_else(|_| "memory".to_string());
    let scheduler: Arc<dyn Scheduler> = match scheduler_type.as_str() {
        "memory" => {
            println!("📦 Using in-memory scheduler");
            Arc::new(MemoryScheduler::new())
        }
        #[cfg(feature = "redis-scheduler")]
        "redis" => {
            let redis_url = env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string());
            println!("📦 Using Redis scheduler at {}", redis_url);
            Arc::new(RedisScheduler::new(&redis_url, "agent-stream", "agent-group", "worker-1", 3).await?)
        }
        _ => {
            anyhow::bail!("Unknown scheduler type: {}", scheduler_type);
        }
    };

    // 2. 选择幂等性存储后端
    let storage_type = env::var("IDEMPOTENCY_STORAGE").unwrap_or_else(|_| "memory".to_string());
    let _idempotency_backend: Arc<dyn IdempotencyBackend> = match storage_type.as_str() {
        "memory" => {
            println!("📦 Using in-memory idempotency backend");
            Arc::new(MemoryBackend::new(1000))
        }
        #[cfg(feature = "sled-storage")]
        "sled" => {
            let path = env::var("SLED_PATH").unwrap_or_else(|_| "./data/sled".to_string());
            println!("📦 Using Sled idempotency backend at {}", path);
            Arc::new(SledBackend::new(&path, 1000)?)
        }
        _ => {
            anyhow::bail!("Unknown idempotency storage type: {}", storage_type);
        }
    };

    // 3. 使用模拟插件
    let plugin: Box<dyn Extension> = Box::new(MockExtension);
    let shared_plugin: Arc<dyn Extension> = Arc::from(plugin);
    let executor = Arc::new(ParallelExecutor::new(shared_plugin.clone()));

    // 4. 启动 worker 循环（消费任务）
    let worker_scheduler = scheduler.clone();
    tokio::spawn(async move {
        loop {
            if let Some(task) = worker_scheduler.pop().await.unwrap() {
                println!("👷 Worker got task: {}", task.id);
                if rand::thread_rng().gen_bool(0.5) {
                    worker_scheduler.ack(&task.id).await.unwrap();
                    println!("✅ Task {} acked", task.id);
                } else {
                    worker_scheduler.nack(&task.id, "simulated error").await.unwrap();
                    println!("❌ Task {} nacked", task.id);
                }
            }

            if std::env::var("DISABLE_SIMULATED_DELAY").is_ok() {
                tokio::task::yield_now().await;
            } else {
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            }
        }
    });

    // 5. 启动 HTTP 服务器，传入 scheduler 和 executor
    println!("🚀 Starting agent-core ingress server...");
    ingress::start_server(scheduler, executor).await?;
    Ok(())
}