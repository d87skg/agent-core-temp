use super::{Scheduler, Task};
use async_trait::async_trait;
use dashmap::DashMap;
use std::cmp::Reverse;
use std::collections::{BinaryHeap, VecDeque};
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::error::{AgentError, Result};

#[derive(Debug, Clone, Eq, PartialEq)]
struct PendingTask {
    task: Task,
    score: i64,
}

impl Ord for PendingTask {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.score.cmp(&other.score)
    }
}

impl PartialOrd for PendingTask {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

pub struct MemoryScheduler {
    pending: Arc<Mutex<BinaryHeap<Reverse<PendingTask>>>>,
    in_progress: Arc<DashMap<String, Task>>,
    dead_letter: Arc<Mutex<VecDeque<Task>>>,
}

impl MemoryScheduler {
    pub fn new() -> Self {
        Self {
            pending: Arc::new(Mutex::new(BinaryHeap::new())),
            in_progress: Arc::new(DashMap::new()),
            dead_letter: Arc::new(Mutex::new(VecDeque::new())),
        }
    }

    fn now_millis() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64
    }

    fn calculate_score(task: &Task) -> i64 {
        let priority_score = (10 - task.priority as i64) * 1_000_000_000_000;
        let time_score = task.delay_until.unwrap_or(0) as i64;
        priority_score + time_score
    }
}

#[async_trait]
impl Scheduler for MemoryScheduler {
    async fn submit(&self, task: Task) -> Result<String> {
        let score = Self::calculate_score(&task);
        let pending = PendingTask { task: task.clone(), score };
        self.pending.lock().await.push(Reverse(pending));
        Ok(task.id)
    }

    async fn pop(&self) -> Result<Option<Task>> {
        let now = Self::now_millis();
        let mut pending = self.pending.lock().await;

        while let Some(Reverse(top)) = pending.peek() {
            if top.task.delay_until.unwrap_or(0) <= now {
                let Reverse(item) = pending.pop().unwrap();
                self.in_progress.insert(item.task.id.clone(), item.task.clone());
                return Ok(Some(item.task));
            } else {
                return Ok(None);
            }
        }
        Ok(None)
    }

    async fn ack(&self, task_id: &str) -> Result<()> {
        self.in_progress
            .remove(task_id)
            .ok_or_else(|| AgentError::NotFound(task_id.to_string()))?;
        Ok(())
    }

    async fn nack(&self, task_id: &str, error: &str) -> Result<()> {
        let (_, mut task) = self.in_progress
            .remove(task_id)
            .ok_or_else(|| AgentError::NotFound(task_id.to_string()))?;

        task.retry_count += 1;
        task.last_error = Some(error.to_string());

        if task.retry_count >= task.max_retries {
            let mut dead = self.dead_letter.lock().await;
            dead.push_back(task);
        } else {
            let backoff_ms = 1000 * (1 << (task.retry_count - 1));
            task.delay_until = Some(Self::now_millis() + backoff_ms as u64);
            let score = Self::calculate_score(&task);
            let pending = PendingTask { task, score };
            self.pending.lock().await.push(Reverse(pending));
        }
        Ok(())
    }

    async fn dead_letter_count(&self) -> Result<usize> {
        Ok(self.dead_letter.lock().await.len())
    }

    async fn purge_expired(&self) -> Result<usize> {
        Ok(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time;

    #[tokio::test]
    async fn test_submit_pop_ack() {
        let scheduler = MemoryScheduler::new();
        let task = Task::new("test", b"payload".to_vec(), 5, None);
        let id = scheduler.submit(task.clone()).await.unwrap();
        assert_eq!(id, task.id);

        let popped = scheduler.pop().await.unwrap().unwrap();
        assert_eq!(popped.id, task.id);

        scheduler.ack(&task.id).await.unwrap();
        assert_eq!(scheduler.in_progress.len(), 0);
    }

    #[tokio::test]
    async fn test_priority() {
        let scheduler = MemoryScheduler::new();
        let low_task = Task::new("low", vec![], 1, None);
        let high_task = Task::new("high", vec![], 10, None);

        scheduler.submit(low_task).await.unwrap();
        scheduler.submit(high_task).await.unwrap();

        let popped1 = scheduler.pop().await.unwrap().unwrap();
        assert_eq!(popped1.task_type, "high");

        let popped2 = scheduler.pop().await.unwrap().unwrap();
        assert_eq!(popped2.task_type, "low");
    }

    #[tokio::test]
    async fn test_delay() {
        let scheduler = MemoryScheduler::new();
        let delay = Duration::from_millis(100);
        let task = Task::new("delayed", vec![], 5, Some(delay));
        scheduler.submit(task).await.unwrap();

        assert!(scheduler.pop().await.unwrap().is_none());

        time::sleep(Duration::from_millis(150)).await;
        let popped = scheduler.pop().await.unwrap().unwrap();
        assert_eq!(popped.task_type, "delayed");
    }

    #[tokio::test]
    async fn test_nack_retry() {
        let scheduler = MemoryScheduler::new();
        let task = Task::new("retry", vec![], 5, None);
        let id = scheduler.submit(task).await.unwrap();

        let popped = scheduler.pop().await.unwrap().unwrap();
        assert_eq!(popped.id, id);

        scheduler.nack(&id, "test error").await.unwrap();

        assert!(scheduler.pop().await.unwrap().is_none());
        time::sleep(Duration::from_millis(1100)).await;
        let retry = scheduler.pop().await.unwrap().unwrap();
        assert_eq!(retry.id, id);
        assert_eq!(retry.retry_count, 1);
        assert_eq!(retry.last_error, Some("test error".to_string()));
    }
}