// src/scheduler/redis_scheduler.rs
use redis::{AsyncCommands, Client, RedisResult};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use anyhow::Result;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: String,
    pub task_type: String,
    pub payload: Vec<u8>,
    pub created_at: u64,
}

pub struct RedisScheduler {
    client: Client,
    queue_key: String,
}

impl RedisScheduler {
    pub async fn new(redis_url: &str) -> Result<Self> {
        let client = Client::open(redis_url)?;
        Ok(Self {
            client,
            queue_key: "agent:tasks".to_string(),
        })
    }

    pub async fn submit(&self, task_type: &str, payload: Vec<u8>) -> Result<String> {
        // 使用新的 multiplexed 连接
        let mut conn = self.client.get_multiplexed_async_connection().await?;
        let task_id = Uuid::new_v4().to_string();
        
        let task = Task {
            id: task_id.clone(),
            task_type: task_type.to_string(),
            payload,
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_secs(),
        };

        let serialized = serde_json::to_string(&task)?;
        // 添加类型注解
        let _: RedisResult<()> = conn.lpush(&self.queue_key, serialized).await;
        Ok(task_id)
    }

    pub async fn pop(&self) -> Result<Option<Task>> {
        // 使用新的 multiplexed 连接
        let mut conn = self.client.get_multiplexed_async_connection().await?;
        let result: Option<String> = conn.rpop(&self.queue_key, None).await?;
        match result {
            Some(json) => Ok(Some(serde_json::from_str(&json)?)),
            None => Ok(None),
        }
    }
}