use anyhow::Result;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex, Semaphore};

// ------------------- 公共接口定义 -------------------

/// 任务句柄，包含任务 ID
pub struct TaskHandle {
    pub id: String,
}

/// 任务状态
#[derive(Debug, Clone, PartialEq)]
pub enum TaskStatus {
    Pending,
    Running,
    Completed,
    Failed(String),
    Cancelled,
}

/// 运行时管理器接口
pub trait RuntimeManager: Send + Sync {
    /// 提交一个异步任务到运行时，返回 TaskHandle
    fn submit<F, Fut>(&self, f: F) -> impl std::future::Future<Output = Result<TaskHandle>> + Send
    where
        F: FnOnce() -> Fut + Send + 'static,
        Fut: Future<Output = Result<()>> + Send;

    /// 获取任务状态
    fn status(&self, task_id: &str)
        -> impl std::future::Future<Output = Option<TaskStatus>> + Send;

    /// 取消任务
    fn cancel(&self, task_id: &str) -> impl std::future::Future<Output = Result<()>> + Send;

    /// 获取当前负载（运行中任务数，队列长度）
    fn load(&self) -> impl std::future::Future<Output = (usize, usize)> + Send;
}

// ------------------- 简单实现 -------------------

/// 一个基于 Tokio 的简单运行时，支持最大并发控制和任务状态跟踪，并支持任务取消
pub struct SimpleRuntime {
    semaphore: Arc<Semaphore>,
    max_concurrent: usize,
    task_tx: mpsc::UnboundedSender<Pin<Box<dyn Future<Output = ()> + Send>>>,
    status_map: Arc<Mutex<HashMap<String, TaskStatus>>>,
    handle_map: Arc<Mutex<HashMap<String, tokio::task::JoinHandle<()>>>>,
}

impl SimpleRuntime {
    /// 创建一个新的 SimpleRuntime
    ///
    /// * `max_concurrent` - 最大并发任务数
    /// * `_queue_size` - 任务队列大小（背压阈值），暂未使用
    pub fn new(max_concurrent: usize, _queue_size: usize) -> Self {
        let semaphore = Arc::new(Semaphore::new(max_concurrent));
        let (tx, mut rx) = mpsc::unbounded_channel();

        // 后台任务：从队列中取出任务并执行（受信号量限制）
        let sem_clone = semaphore.clone();
        tokio::spawn(async move {
            while let Some(task) = rx.recv().await {
                let permit = sem_clone.clone().acquire_owned().await.unwrap();
                tokio::spawn(async move {
                    task.await;
                    drop(permit); // 释放信号量
                });
            }
        });

        Self {
            semaphore,
            max_concurrent,
            task_tx: tx,
            status_map: Arc::new(Mutex::new(HashMap::new())),
            handle_map: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

impl RuntimeManager for SimpleRuntime {
    async fn submit<F, Fut>(&self, f: F) -> Result<TaskHandle>
    where
        F: FnOnce() -> Fut + Send + 'static,
        Fut: Future<Output = Result<()>> + Send,
    {
        let task_id = uuid::Uuid::new_v4().to_string();

        // 初始状态为 Pending
        {
            let mut map = self.status_map.lock().await;
            map.insert(task_id.clone(), TaskStatus::Pending);
        }

        let status_map = self.status_map.clone();
        let handle_map = self.handle_map.clone();
        let task_id_clone = task_id.clone();

        // 包装任务，以便更新状态和存储句柄
        let task = async move {
            // 克隆 ID 用于后续移除操作
            let id_for_remove = task_id_clone.clone();

            // 更新为 Running
            {
                let mut map = status_map.lock().await;
                map.insert(task_id_clone.clone(), TaskStatus::Running);
            }

            // 执行用户任务
            let result = f().await;

            // 检查是否已经被取消（通过 handle_map 是否存在来判断）
            let cancelled = {
                let map = handle_map.lock().await;
                !map.contains_key(&task_id_clone)
            };

            if cancelled {
                let mut map = status_map.lock().await;
                map.insert(task_id_clone, TaskStatus::Cancelled);
            } else {
                let mut map = status_map.lock().await;
                if let Err(e) = result {
                    map.insert(task_id_clone, TaskStatus::Failed(e.to_string()));
                } else {
                    map.insert(task_id_clone, TaskStatus::Completed);
                }
                // 任务正常结束，移除 handle（使用克隆的 ID）
                let mut hmap = handle_map.lock().await;
                hmap.remove(&id_for_remove);
            }
        };

        // 启动任务并存储句柄
        let join_handle = tokio::spawn(task);
        {
            let mut hmap = self.handle_map.lock().await;
            hmap.insert(task_id.clone(), join_handle);
        }

        self.task_tx
            .send(Box::pin(async {}))
            .map_err(|_| anyhow::anyhow!("runtime stopped"))?;
        Ok(TaskHandle { id: task_id })
    }

    async fn status(&self, task_id: &str) -> Option<TaskStatus> {
        let map = self.status_map.lock().await;
        map.get(task_id).cloned()
    }

    async fn cancel(&self, task_id: &str) -> Result<()> {
        let handle = {
            let mut hmap = self.handle_map.lock().await;
            hmap.remove(task_id)
        };

        if let Some(handle) = handle {
            handle.abort(); // 终止任务
                            // 更新状态为 Cancelled
            let mut smap = self.status_map.lock().await;
            smap.insert(task_id.to_string(), TaskStatus::Cancelled);
            Ok(())
        } else {
            // 任务可能已完成或不存在
            Err(anyhow::anyhow!("task not found or already finished"))
        }
    }

    async fn load(&self) -> (usize, usize) {
        let available = self.semaphore.available_permits();
        let running = self.max_concurrent - available;
        (running, 0)
    }
}
