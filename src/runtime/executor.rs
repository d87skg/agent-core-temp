// src/runtime/executor.rs

use crate::idempotency::manager::UltimateIdempotencyManager;
use crate::observability::metrics;
use anyhow::Result;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::Semaphore;

/// 任务定义，包含幂等性所需的字段
#[derive(Debug, Clone)]
pub struct Task {
    pub id: String,
    pub workflow_id: String,
    pub step_id: String,
    pub intent: String,
    pub payload: Vec<u8>,
}

/// 任务执行结果
#[derive(Debug, Clone)]
pub struct TaskResult {
    pub task_id: String,
    pub output: Vec<u8>,
}

/// 并行执行器，集成幂等性保证
pub struct ParallelExecutor {
    worker_id: String,
    max_concurrency: usize,
    idempotency_manager: Arc<UltimateIdempotencyManager>,
    semaphore: Arc<Semaphore>,
}

impl ParallelExecutor {
    /// 创建一个新的执行器
    /// - worker_id: 当前工作节点标识
    /// - max_concurrency: 最大并发任务数
    /// - idempotency_manager: 幂等性管理器（共享）
    pub fn new(
        worker_id: String,
        max_concurrency: usize,
        idempotency_manager: Arc<UltimateIdempotencyManager>,
    ) -> Self {
        Self {
            worker_id,
            max_concurrency,
            idempotency_manager,
            semaphore: Arc::new(Semaphore::new(max_concurrency)),
        }
    }

    /// 执行单个任务，自动应用幂等性
    pub async fn execute_task(&self, task: Task) -> Result<TaskResult> {
        // 获取并发许可（限流）
        let _permit = self.semaphore.acquire().await?;

        let start = Instant::now();
        metrics::inc_tasks_total();

        // 使用幂等性管理器执行任务闭包
        let result = self
            .idempotency_manager
            .execute(
                &task.workflow_id,
                &task.step_id,
                &task.intent,
                || async {
                    // 这里是真正的业务逻辑，例如处理 payload
                    // 此处仅作示例，模拟耗时操作
                    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
                    Ok::<_, anyhow::Error>(task.payload.clone())
                },
            )
            .await?;

        metrics::inc_tasks_success();
        metrics::observe_task_duration(start.elapsed().as_secs_f64());

        Ok(TaskResult {
            task_id: task.id,
            output: result,
        })
    }

    /// 批量执行任务，控制并发
    pub async fn execute_batch(&self, tasks: Vec<Task>) -> Vec<Result<TaskResult>> {
        use futures::stream::{self, StreamExt};
        stream::iter(tasks)
            .map(|task| async { self.execute_task(task).await })
            .buffer_unordered(self.max_concurrency)
            .collect()
            .await
    }
}