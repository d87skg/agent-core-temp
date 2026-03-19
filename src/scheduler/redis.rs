//! Redis-based distributed task scheduler using Redis Streams.
//!
//! This scheduler provides reliable task delivery with at-least-once semantics,
//! automatic retries, and a dead letter queue for failed tasks. It exposes
//! Prometheus metrics for monitoring.
//!
//! # Reliability guarantees
//! - Atomic NACK with XACK and XADD using pipeline.
//! - Tasks exceeding `max_retries` are moved to dead letter queue.
//! - Metrics for monitoring task flow and failures.
use super::{Scheduler, Task};
use async_trait::async_trait;
use dashmap::DashMap;
use redis::aio::ConnectionManager;
use redis::{AsyncCommands, Client, RedisResult, pipe};
use serde_json;
use std::sync::Arc;
use tracing::{debug, error, info, warn};

use crate::error::{AgentError, Result};
use crate::observability;

pub struct RedisScheduler {
    conn: ConnectionManager,
    stream_key: String,
    dead_letter_key: String,
    consumer_group: String,
    consumer_name: String,
    max_retries: u32,
    pending_tasks: Arc<DashMap<String, (String, Task)>>,
}

impl RedisScheduler {
    pub async fn new(
        redis_url: &str,
        stream_key: &str,
        consumer_group: &str,
        consumer_name: &str,
        max_retries: u32,
    ) -> Result<Self> {
        let client = Client::open(redis_url)
            .map_err(|e| AgentError::Queue(format!("Redis connection error: {}", e)))?;
        let conn = ConnectionManager::new(client).await
            .map_err(|e| AgentError::Queue(format!("Redis connection manager error: {}", e)))?;

        let _: RedisResult<()> = redis::cmd("XGROUP")
            .arg("CREATE")
            .arg(stream_key)
            .arg(consumer_group)
            .arg("0")
            .arg("MKSTREAM")
            .query_async(&mut conn.clone())
            .await;

        Ok(Self {
            conn,
            stream_key: stream_key.to_string(),
            dead_letter_key: format!("{}:dead", stream_key),
            consumer_group: consumer_group.to_string(),
            consumer_name: consumer_name.to_string(),
            max_retries,
            pending_tasks: Arc::new(DashMap::new()),
        })
    }

    fn serialize_task(task: &Task) -> Result<String> {
        serde_json::to_string(task).map_err(|e| AgentError::Serialization(e))
    }

    fn deserialize_task(data: &str) -> Result<Task> {
        serde_json::from_str(data).map_err(|e| AgentError::Serialization(e))
    }
}

#[async_trait]
impl Scheduler for RedisScheduler {
    async fn submit(&self, task: Task) -> Result<String> {
        let mut conn = self.conn.clone();
        let serialized = Self::serialize_task(&task)?;
        // 显式指定 xadd 的返回类型为 ()，解决 never type 回退错误
        conn.xadd::<_, _, _, _, ()>(&self.stream_key, "*", &[("task", serialized)])
            .await
            .map_err(|e| AgentError::Queue(format!("Redis XADD error: {}", e)))?;
        observability::REDIS_TASKS_SUBMITTED.increment(1);
        debug!("Submitted task: {}", task.id);
        Ok(task.id)
    }

    async fn pop(&self) -> Result<Option<Task>> {
        let mut conn = self.conn.clone();

        // 1. 处理 pending 消息
        let pending_result: RedisResult<Vec<(String, Vec<(String, Vec<(String, String)>)>)>> =
            redis::cmd("XREADGROUP")
                .arg("GROUP")
                .arg(&self.consumer_group)
                .arg(&self.consumer_name)
                .arg("COUNT")
                .arg(1)
                .arg("STREAMS")
                .arg(&self.stream_key)
                .arg("0")
                .query_async(&mut conn)
                .await;

        match pending_result {
            Ok(streams) => {
                for (_stream, entries) in streams {
                    for (entry_id, fields) in entries {
                        for (key, val) in fields {
                            if key == "task" {
                                let task = Self::deserialize_task(&val)?;
                                self.pending_tasks.insert(task.id.clone(), (entry_id, task.clone()));
                                observability::REDIS_TASKS_POPPED.increment(1);
                                return Ok(Some(task));
                            }
                        }
                    }
                }
            }
            Err(e) => {
                observability::REDIS_OPERATION_FAILURES.increment(1);
                warn!("Error reading pending messages: {}", e);
            }
        }

        // 2. 读取新消息
        let new_result: RedisResult<Vec<(String, Vec<(String, Vec<(String, String)>)>)>> =
            redis::cmd("XREADGROUP")
                .arg("GROUP")
                .arg(&self.consumer_group)
                .arg(&self.consumer_name)
                .arg("COUNT")
                .arg(1)
                .arg("BLOCK")
                .arg(100)
                .arg("STREAMS")
                .arg(&self.stream_key)
                .arg(">")
                .query_async(&mut conn)
                .await;

        match new_result {
            Ok(streams) => {
                for (_stream, entries) in streams {
                    for (entry_id, fields) in entries {
                        for (key, val) in fields {
                            if key == "task" {
                                let task = Self::deserialize_task(&val)?;
                                self.pending_tasks.insert(task.id.clone(), (entry_id, task.clone()));
                                observability::REDIS_TASKS_POPPED.increment(1);
                                return Ok(Some(task));
                            }
                        }
                    }
                }
                Ok(None)
            }
            Err(e) => {
                observability::REDIS_OPERATION_FAILURES.increment(1);
                warn!("XREADGROUP error: {}", e);
                Ok(None)
            }
        }
    }

    async fn ack(&self, task_id: &str) -> Result<()> {
        if let Some((_, (message_id, _))) = self.pending_tasks.remove(task_id) {
            let mut conn = self.conn.clone();
            let num_acked: usize = conn
                .xack(&self.stream_key, &self.consumer_group, &[message_id.as_str()])
                .await
                .map_err(|e| AgentError::Queue(format!("Redis XACK error: {}", e)))?;
            if num_acked == 1 {
                observability::REDIS_TASKS_ACKED.increment(1);
                debug!("ACK success for task {}, message ID {}", task_id, message_id);
            } else {
                error!("XACK expected 1, got {} for task {} message ID {}", num_acked, task_id, message_id);
                return Err(AgentError::Queue(format!("XACK expected 1, got {}", num_acked)));
            }
        } else {
            warn!("ack: task {} not found in pending_tasks", task_id);
        }
        Ok(())
    }

    async fn nack(&self, task_id: &str, error_msg: &str) -> Result<()> {
        if let Some((_, (message_id, mut task))) = self.pending_tasks.remove(task_id) {
            let mut conn = self.conn.clone();

            task.retry_count += 1;
            task.last_error = Some(error_msg.to_string());
            info!("Task {} nacked, retry count now {}", task_id, task.retry_count);

            let serialized = Self::serialize_task(&task)?;
            let mut pipe = pipe();

            // 始终 XACK 原消息
            pipe.xack(&self.stream_key, &self.consumer_group, &[message_id.as_str()]);

            if task.retry_count >= self.max_retries {
                pipe.xadd(&self.dead_letter_key, "*", &[("task", serialized.as_str())]);
            } else {
                pipe.xadd(&self.stream_key, "*", &[("task", serialized.as_str())]);
            }

            let results: (usize, String) = pipe
                .query_async(&mut conn)
                .await
                .map_err(|e| AgentError::Queue(format!("Redis pipeline error: {}", e)))?;

            let (acked, new_message_id) = results;
            if acked != 1 {
                error!("XACK expected 1, got {} for task {} message ID {}", acked, task_id, message_id);
            }

            observability::REDIS_TASKS_NACKED.increment(1);
            if task.retry_count >= self.max_retries {
                info!("Task {} moved to dead letter with new ID {}", task_id, new_message_id);
            } else {
                debug!("Re-submitted task {} with new message ID {}", task_id, new_message_id);
            }
        } else {
            warn!("nack: task {} not found in pending_tasks", task_id);
        }
        Ok(())
    }

    async fn dead_letter_count(&self) -> Result<usize> {
        let mut conn = self.conn.clone();
        let len: u64 = conn
            .xlen(&self.dead_letter_key)
            .await
            .map_err(|e| AgentError::Queue(format!("Redis XLEN error: {}", e)))?;
        observability::REDIS_DEAD_LETTER_SIZE.set(len as f64);
        Ok(len as usize)
    }

    async fn purge_expired(&self) -> Result<usize> {
        Ok(0)
    }
}