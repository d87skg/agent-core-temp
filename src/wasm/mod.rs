use async_trait::async_trait;
use serde_json::Value;
use std::time::Duration;

pub type Result<T> = std::result::Result<T, WasmError>;

#[derive(Debug, thiserror::Error)]
pub enum WasmError {
    #[error("Module compilation failed: {0}")]
    CompilationFailed(String),
    #[error("Function execution failed: {0}")]
    ExecutionFailed(String),
    #[error("Out of fuel")]
    OutOfFuel,
    #[error("Memory limit exceeded")]
    MemoryLimitExceeded,
    #[error("Timeout")]
    Timeout,
    #[error("Host function error: {0}")]
    HostFunctionError(String),
}

#[derive(Debug, Clone)]
pub struct ModuleHandle {
    pub id: String,
    pub name: String,
    pub version: String,
}

#[derive(Debug, Clone)]
pub struct ExecutionStats {
    pub fuel_consumed: u64,
    pub memory_used: usize,
    pub time_elapsed: Duration,
}

#[async_trait]
pub trait WasmSandbox: Send + Sync + 'static {
    type Handle: Send + Sync;

    async fn load(
        &self,
        name: &str,
        wasm_bytes: &[u8],
        memory_limit: usize,
        fuel_limit: Option<u64>,
        allowed_host_functions: Vec<&str>,
    ) -> Result<Self::Handle>;

    async fn call(
        &self,
        handle: &Self::Handle,
        function_name: &str,
        arguments: Value,
        fuel_allocation: Option<u64>,
        timeout: Option<Duration>,
    ) -> Result<(Value, ExecutionStats)>;

    async fn get_stats(&self, handle: &Self::Handle) -> Result<ExecutionStats>;
}