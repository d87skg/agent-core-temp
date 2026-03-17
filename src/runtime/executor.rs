use anyhow::Result;
use std::sync::Arc;
use tokio::sync::Semaphore as TokioSemaphore;
use std::sync::RwLock;
use std::collections::HashMap;
use crate::observability;

pub struct ParallelExecutor {
    #[allow(dead_code)]
    max_concurrent: usize,
    semaphore: Arc<TokioSemaphore>,
    tasks: Arc<RwLock<HashMap<String, TaskHandle>>>,
}

#[allow(dead_code)]
struct TaskHandle {
    id: String,
    status: TaskStatus,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TaskStatus {
    Pending,
    Running,
    Completed,
    Failed,
}

impl ParallelExecutor {
    pub fn new(max_concurrent: usize) -> Self {
        observability::set_tasks_running(0);
        Self {
            max_concurrent,
            semaphore: Arc::new(TokioSemaphore::new(max_concurrent)),
            tasks: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn execute<F, T>(&self, task_id: String, f: F) -> Result<T>
    where
        F: FnOnce() -> T + Send + 'static,
        T: Send + 'static,
    {
        observability::increment_tasks_total();

        let _permit = self.semaphore.acquire().await?;

        {
            let mut tasks = self.tasks.write().unwrap();
            tasks.insert(task_id.clone(), TaskHandle {
                id: task_id.clone(),
                status: TaskStatus::Running,
            });
        }

        observability::set_tasks_running(self.get_running_tasks().len() as u64);

        let start = std::time::Instant::now();
        let result = tokio::task::spawn_blocking(f).await;
        let duration = start.elapsed();

        observability::record_request_duration(duration.as_secs_f64());

        {
            let mut tasks = self.tasks.write().unwrap();
            if let Some(handle) = tasks.get_mut(&task_id) {
                handle.status = TaskStatus::Completed;
            }
        }

        observability::set_tasks_running(self.get_running_tasks().len() as u64);
        observability::increment_tasks_completed();

        Ok(result?)
    }

    pub fn get_status(&self, task_id: &str) -> Option<TaskStatus> {
        let tasks = self.tasks.read().unwrap();
        tasks.get(task_id).map(|h| h.status.clone())
    }

    pub fn get_running_tasks(&self) -> Vec<String> {
        let tasks = self.tasks.read().unwrap();
        tasks
            .iter()
            .filter(|(_, h)| h.status == TaskStatus::Running)
            .map(|(id, _)| id.clone())
            .collect()
    }

    pub fn get_completed_tasks(&self) -> Vec<String> {
        let tasks = self.tasks.read().unwrap();
        tasks
            .iter()
            .filter(|(_, h)| h.status == TaskStatus::Completed)
            .map(|(id, _)| id.clone())
            .collect()
    }

    pub fn available_permits(&self) -> usize {
        self.semaphore.available_permits()
    }
}