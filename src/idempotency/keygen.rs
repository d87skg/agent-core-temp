use sha2::{Sha256, Digest};

pub struct IdempotencyKeyGenerator;

impl IdempotencyKeyGenerator {
    pub fn new() -> Self {
        Self
    }

    pub fn generate(&self, workflow_id: &str, step_id: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(workflow_id.as_bytes());
        hasher.update(step_id.as_bytes());
        // 添加固定的盐，确保唯一性
        hasher.update("agent-core-salt".as_bytes());
        let result = hasher.finalize();
        hex::encode(&result[..16]) // 取前16字节作为幂等键
    }
}