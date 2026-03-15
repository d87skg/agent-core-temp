// src/main.rs
use agent_core_temp::observability;
use agent_core_temp::ingress;
use agent_core_temp::scheduler::RedisScheduler;
use agent_core_temp::runtime::executor::ParallelExecutor;
use anyhow::Result;
use std::sync::Arc;
use tokio::time::{sleep, Duration};
use tracing_subscriber;

#[tokio::main]
async fn main() -> Result<()> {
    // 初始化日志
    tracing_subscriber::fmt()
        .with_env_filter("info")
        .with_target(true)
        .init();

    println!("🚀 Initializing observability...");
    observability::init_observability()?;
    println!("✅ Observability initialized");

    // 初始化 Redis 调度器
    let redis_url = "redis://127.0.0.1:6379";
    let scheduler = Arc::new(RedisScheduler::new(redis_url).await?);
    println!("✅ Redis scheduler connected to {}", redis_url);

    // 创建并发执行器
    let executor = Arc::new(ParallelExecutor::new(10));

    // 启动后台 worker 循环
    let worker_scheduler = scheduler.clone();
    let worker_executor = executor.clone();
    let _worker_handle = tokio::spawn(async move {
        loop {
            match worker_scheduler.pop().await {
                Ok(Some(task)) => {
                    println!("📥 Worker received task: {} (type: {})", task.id, task.task_type);
                    observability::increment_tasks_total();

                    let task_id_for_exec = task.id.clone();
                    let task_id_for_log = task.id.clone();
                    let task_type_for_exec = task.task_type.clone();
                    let payload_for_exec = task.payload.clone();

                    let exec = worker_executor.clone();
                    tokio::spawn(async move {
                        let result = exec.execute(task_id_for_exec, move || {
                            println!("⚙️  Executing task of type {}", task_type_for_exec);
                            std::thread::sleep(Duration::from_millis(500));
                            payload_for_exec.len()
                        }).await;

                        match result {
                            Ok(len) => {
                                println!("✅ Task {} completed, result length: {}", task_id_for_log, len);
                                observability::increment_tasks_completed();
                            }
                            Err(e) => {
                                eprintln!("❌ Task {} failed: {}", task_id_for_log, e);
                                observability::increment_tasks_failed();
                            }
                        }
                    });
                }
                Ok(None) => sleep(Duration::from_millis(100)).await,
                Err(e) => {
                    eprintln!("⚠️  Error popping task: {}", e);
                    sleep(Duration::from_secs(1)).await;
                }
            }
        }
    });

    // 临时测试任务
    let test_task_id = scheduler.submit("test", b"hello".to_vec()).await?;
    println!("📤 Submitted test task: {}", test_task_id);

    println!("🚀 Starting agent-core ingress server...");
    // 传入 scheduler 参数
    let ingress_future = ingress::start_server(scheduler.clone());

    tokio::select! {
        result = ingress_future => {
            if let Err(e) = result {
                eprintln!("Ingress server error: {}", e);
            }
        }
        _ = tokio::signal::ctrl_c() => {
            println!("👋 Received Ctrl+C, shutting down gracefully...");
        }
    }

    Ok(())
}