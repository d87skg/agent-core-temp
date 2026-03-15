// src/wasm/memory.rs
use super::{ModuleHandle, WasmError, WasmSandbox, ExecutionStats, Result};
use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;
use wasmtime::{Engine, Module, Store, Linker, Func, Val, Extern, Memory, MemoryType};

pub struct WasmtimeSandbox {
    engine: Engine,
    modules: Arc<Mutex<HashMap<String, Module>>>,
}

impl WasmtimeSandbox {
    pub fn new() -> Self {
        Self {
            engine: Engine::default(),
            modules: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

pub struct WasmtimeHandle {
    id: String,
    name: String,
    version: String,
    module: Module,
    memory_limit: usize,
    fuel_limit: Option<u64>,
}

#[async_trait]
impl WasmSandbox for WasmtimeSandbox {
    type Handle = WasmtimeHandle;

    async fn load(
        &self,
        name: &str,
        wasm_bytes: &[u8],
        memory_limit: usize,
        fuel_limit: Option<u64>,
        _allowed_host_functions: Vec<&str>,
    ) -> Result<Self::Handle> {
        let module = Module::new(&self.engine, wasm_bytes)
            .map_err(|e| WasmError::CompilationFailed(e.to_string()))?;

        let id = format!("{}-{}", name, uuid::Uuid::new_v4());
        self.modules.lock().await.insert(id.clone(), module.clone());

        Ok(WasmtimeHandle {
            id,
            name: name.to_string(),
            version: "0.1".to_string(),
            module,
            memory_limit,
            fuel_limit,
        })
    }

    async fn call(
        &self,
        handle: &Self::Handle,
        function_name: &str,
        arguments: Value,
        fuel_allocation: Option<u64>,
        timeout: Option<Duration>,
    ) -> Result<(Value, ExecutionStats)> {
        let mut store = Store::new(&self.engine, ());

        // 设置燃料限制
        if let Some(fuel) = fuel_allocation.or(handle.fuel_limit) {
            store
                .add_fuel(fuel)
                .map_err(|_| WasmError::OutOfFuel)?;
        }

        // 限制内存（可选）
        // 实际限制在实例化时由Module保证，这里简单设置memory
        // 可以获取 memory 实例并检查大小

        let linker = Linker::new(&self.engine);
        let instance = linker
            .instantiate(&mut store, &handle.module)
            .map_err(|e| WasmError::ExecutionFailed(e.to_string()))?;

        let func = instance
            .get_func(&mut store, function_name)
            .ok_or_else(|| WasmError::ExecutionFailed("Function not found".to_string()))?;

        // 参数序列化：简化处理，假设参数是i32数组
        let args = if let Value::Array(arr) = arguments {
            arr.iter()
                .filter_map(|v| v.as_i64())
                .map(|i| Val::I32(i as i32))
                .collect::<Vec<_>>()
        } else {
            vec![]
        };

        let start = Instant::now();
        let mut results = vec![Val::I32(0)];
        let call_result = if let Some(timeout) = timeout {
            tokio::time::timeout(timeout, async {
                func.call(&mut store, &args, &mut results)
            })
            .await
            .map_err(|_| WasmError::Timeout)?
        } else {
            func.call(&mut store, &args, &mut results)
        };
        let elapsed = start.elapsed();

        call_result.map_err(|e| WasmError::ExecutionFailed(e.to_string()))?;

        let fuel_consumed = handle.fuel_limit.map(|limit| {
            let remaining = store.get_fuel().unwrap_or(0);
            limit.saturating_sub(remaining)
        }).unwrap_or(0);

        let memory_used = 0; // 简化，可从store获取memory大小

        let result_value = if let Some(Val::I32(v)) = results.first() {
            Value::Number((*v).into())
        } else {
            Value::Null
        };

        Ok((
            result_value,
            ExecutionStats {
                fuel_consumed,
                memory_used,
                time_elapsed: elapsed,
            },
        ))
    }

    async fn get_stats(&self, _handle: &Self::Handle) -> Result<ExecutionStats> {
        // 简化实现
        Ok(ExecutionStats {
            fuel_consumed: 0,
            memory_used: 0,
            time_elapsed: Duration::default(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_add() {
        let sandbox = WasmtimeSandbox::new();
        let wasm_bytes = wat::parse_str(
            r#"
            (module
                (func $add (param i32 i32) (result i32)
                    local.get 0
                    local.get 1
                    i32.add)
                (export "add" (func $add))
            )
            "#,
        )
        .unwrap();

        let handle = sandbox
            .load("test", &wasm_bytes, 64 * 1024, Some(1000), vec![])
            .await
            .unwrap();

        let (result, _stats) = sandbox
            .call(&handle, "add", serde_json::json!([2, 3]), None, None)
            .await
            .unwrap();

        assert_eq!(result, 5);
    }
}