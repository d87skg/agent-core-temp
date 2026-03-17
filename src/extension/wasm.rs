use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use wasmtime::{Engine, Linker, Module, Store, Instance, Config, Val}; // 添加 Val 导入
use wasmtime::ResourceLimiter;
use serde_json::Value;

use super::Extension;

pub struct HostState {
    pub memory_limit: usize,
    pub fuel_limit: u64,
    pub storage: Arc<Mutex<HashMap<String, String>>>,
    pub logger: Arc<dyn Fn(&str, u32) + Send + Sync>,
    pub http_allowlist: Vec<String>,
    pub workspace_root: Option<PathBuf>,
    pub max_body_size: usize,
}

impl ResourceLimiter for HostState {
    fn memory_growing(&mut self, current: usize, desired: usize, _maximum: Option<usize>) -> anyhow::Result<bool> {
        if desired > self.memory_limit {
            eprintln!("Memory limit exceeded: current={}, desired={}, limit={}", current, desired, self.memory_limit);
            Ok(false)
        } else {
            Ok(true)
        }
    }

    fn table_growing(&mut self, _current: usize, desired: usize, _maximum: Option<usize>) -> anyhow::Result<bool> {
        Ok(desired < 1024)
    }
}

fn host_log(
    mut caller: wasmtime::Caller<'_, HostState>,
    level: u32,
    msg_ptr: u32,
    msg_len: u32,
) {
    let mem = match caller.get_export("memory") {
        Some(wasmtime::Extern::Memory(mem)) => mem,
        _ => {
            eprintln!("host_log: cannot get memory");
            return;
        }
    };

    let data = mem.data(&caller);
    let start = msg_ptr as usize;
    let end = start + msg_len as usize;

    if end > data.len() {
        eprintln!("host_log: message out of bounds");
        return;
    }

    let msg = String::from_utf8_lossy(&data[start..end]);
    (caller.data().logger)(&msg, level);
}

fn host_storage_get(
    mut caller: wasmtime::Caller<'_, HostState>,
    key_ptr: u32,
    key_len: u32,
    ret_ptr: u32,
    ret_len_ptr: u32,
) -> i32 {
    let mem = match caller.get_export("memory") {
        Some(wasmtime::Extern::Memory(mem)) => mem,
        _ => return -1,
    };

    let data = mem.data(&caller);
    let key_start = key_ptr as usize;
    let key_end = key_start + key_len as usize;

    if key_end > data.len() {
        return -1;
    }

    let key = String::from_utf8_lossy(&data[key_start..key_end]).to_string();
    let storage = caller.data().storage.lock().unwrap();
    let value = storage.get(&key).cloned().unwrap_or_default();
    let bytes = value.into_bytes();

    let ret_start = ret_ptr as usize;
    let ret_end = ret_start + bytes.len();

    if ret_end > data.len() {
        return -1;
    }

    unsafe {
        std::ptr::copy_nonoverlapping(
            bytes.as_ptr(),
            data.as_ptr().add(ret_start) as *mut u8,
            bytes.len(),
        );
    }

    let len_ptr = ret_len_ptr as usize;
    if len_ptr + 4 <= data.len() {
        let len_bytes = (bytes.len() as u32).to_le_bytes();
        unsafe {
            std::ptr::copy_nonoverlapping(
                len_bytes.as_ptr(),
                data.as_ptr().add(len_ptr) as *mut u8,
                4,
            );
        }
    }

    0
}

fn host_storage_set(
    mut caller: wasmtime::Caller<'_, HostState>,
    key_ptr: u32,
    key_len: u32,
    val_ptr: u32,
    val_len: u32,
) -> i32 {
    let mem = match caller.get_export("memory") {
        Some(wasmtime::Extern::Memory(mem)) => mem,
        _ => return -1,
    };

    let data = mem.data(&caller);
    let key_start = key_ptr as usize;
    let key_end = key_start + key_len as usize;
    let val_start = val_ptr as usize;
    let val_end = val_start + val_len as usize;

    if key_end > data.len() || val_end > data.len() {
        return -1;
    }

    let key = String::from_utf8_lossy(&data[key_start..key_end]).to_string();
    let val = String::from_utf8_lossy(&data[val_start..val_end]).to_string();

    caller.data().storage.lock().unwrap().insert(key, val);
    0
}

// ====================== 辅助函数 ======================
fn read_string_from_memory(
    mem: &wasmtime::Memory,
    store: &impl wasmtime::AsContext,
    ptr: u32,
    len: u32,
) -> Result<String, i32> {
    let data = mem.data(store);
    let start = ptr as usize;
    let end = start + len as usize;
    if end > data.len() {
        return Err(-1);
    }
    Ok(String::from_utf8_lossy(&data[start..end]).to_string())
}

/// 同步 HTTP GET 宿主函数（简化版：返回模拟数据，且返回 i32 0）
fn host_http_get(
    mut caller: wasmtime::Caller<'_, HostState>,
    url_ptr: u32,
    url_len: u32,
    _headers_ptr: u32,
    _headers_len: u32,
    ret_ptr: u32,
    ret_len_ptr: u32,
) -> Result<i32, wasmtime::Error> {
    let mem = match caller.get_export("memory") {
        Some(wasmtime::Extern::Memory(mem)) => mem,
        _ => {
            eprintln!("host_http_get: cannot get memory");
            return Err(wasmtime::Error::msg("no memory"));
        }
    };

    let url = match read_string_from_memory(&mem, &caller, url_ptr, url_len) {
        Ok(s) => s,
        Err(_) => return Err(wasmtime::Error::msg("invalid URL string")),
    };

    let host_state = caller.data();
    let allowed = host_state.http_allowlist.iter().any(|prefix| url.starts_with(prefix));
    if !allowed {
        eprintln!("HTTP URL not allowed: {}", url);
        return Err(wasmtime::Error::msg("URL not allowed"));
    }

    // 模拟返回数据（一个简单的 JSON 字符串）
    let mock_body = format!(r#"{{"url":"{}","method":"GET","origin":"127.0.0.1"}}"#, url);
    let body = mock_body.into_bytes();

    if body.len() > host_state.max_body_size {
        eprintln!("Response body too large ({} > {})", body.len(), host_state.max_body_size);
        return Err(wasmtime::Error::msg("response too large"));
    }

    let data = mem.data_mut(&mut caller);
    let ret_start = ret_ptr as usize;
    let ret_end = ret_start + body.len();
    if ret_end > data.len() {
        eprintln!("Return buffer too small");
        return Err(wasmtime::Error::msg("buffer too small"));
    }

    data[ret_start..ret_end].copy_from_slice(&body);

    // 写入长度
    let len_ptr = ret_len_ptr as usize;
    if len_ptr + 4 <= data.len() {
        let len_bytes = (body.len() as u32).to_le_bytes();
        data[len_ptr..len_ptr+4].copy_from_slice(&len_bytes);
    }

    Ok(0) // 返回 0 表示成功
}

// ====================== WasmExtension ======================
pub struct WasmExtension {
    name: String,
    version: String,
    instance: Instance,
    store: Mutex<Store<HostState>>,
}

impl WasmExtension {
    pub fn new(
        name: String,
        version: String,
        instance: Instance,
        store: Store<HostState>,
    ) -> Self {
        Self {
            name,
            version,
            instance,
            store: Mutex::new(store),
        }
    }
}

impl Extension for WasmExtension {
    fn name(&self) -> &str {
        &self.name
    }

    fn version(&self) -> &str {
        &self.version
    }

    fn init(&self) -> Result<(), Box<dyn std::error::Error>> {
        let mut guard = self.store.lock().unwrap();
        if let Some(func) = self.instance.get_func(&mut *guard, "_init") {
            let mut results = vec![];
            func.call(&mut *guard, &[], &mut results)?;
        }
        Ok(())
    }

    fn call(&self, method: &str, _payload: Value) -> Result<Value, Box<dyn std::error::Error>> {
        let mut guard = self.store.lock().unwrap();

        let start = std::time::Instant::now();
        let initial_fuel = guard.get_fuel().unwrap_or(0);

        let fuel_limit = guard.data().fuel_limit;
        guard.set_fuel(fuel_limit)?;

        let func = self.instance
            .get_func(&mut *guard, method)
            .ok_or_else(|| format!("method {} not found", method))?;

        // 检查函数返回值类型，这里简单假设所有导出函数都返回 i32
        let mut results = vec![wasmtime::Val::I32(0)];
        let result = func.call(&mut *guard, &[], &mut results);

        let duration = start.elapsed();
        let fuel_used = initial_fuel.saturating_sub(guard.get_fuel().unwrap_or(0));
        let _memory_mb = guard.data().memory_limit as f64 / (1024.0 * 1024.0);

        match result {
            Ok(_) => {
                crate::observability::record_wasm_fuel(fuel_used, &self.name);
                crate::observability::record_wasm_duration(
                    duration.as_millis() as f64,
                    &self.name,
                    &self.version
                );
                // 将返回的 i32 转换为 Value
                if let Some(wasmtime::Val::I32(val)) = results.first() {
                    Ok(Value::Number((*val).into()))
                } else {
                    Ok(Value::Null)
                }
            }
            Err(e) => {
                crate::observability::record_wasm_error("execution_failed", &self.name);
                Err(anyhow::anyhow!(e).into())
            }
        }
    }
}

// ====================== Loader ======================
pub fn load_wasm_plugin(
    path: &str,
    name: String,
    version: String,
    mem_limit: usize,
    cpu_fuel: u64,
    logger: Arc<dyn Fn(&str, u32) + Send + Sync>,
    http_allowlist: Vec<String>,
    workspace_root: Option<PathBuf>,
    max_body_size: usize,
) -> Result<Box<dyn Extension>, Box<dyn std::error::Error>> {
    let mut config = Config::new();
    config.consume_fuel(true);
    let engine = Engine::new(&config)?;

    let module = Module::from_file(&engine, path)?;

    let mut linker = Linker::new(&engine);
    linker.func_wrap("host", "log", host_log)?;
    linker.func_wrap("host", "storage_get", host_storage_get)?;
    linker.func_wrap("host", "storage_set", host_storage_set)?;

    // 注册 http_get，返回 i32
    linker.func_wrap(
        "host",
        "http_get",
        host_http_get,
    )?;

    let host_state = HostState {
        memory_limit: mem_limit,
        fuel_limit: cpu_fuel,
        storage: Arc::new(Mutex::new(HashMap::new())),
        logger,
        http_allowlist,
        workspace_root,
        max_body_size,
    };

    let mut store = Store::new(&engine, host_state);
    store.set_fuel(cpu_fuel)?;
    store.limiter(|state| state as &mut dyn ResourceLimiter);

    let instance = linker.instantiate(&mut store, &module)?;

    crate::observability::record_wasm_plugin_loaded(&name, &version);

    Ok(Box::new(WasmExtension::new(name, version, instance, store)))
}