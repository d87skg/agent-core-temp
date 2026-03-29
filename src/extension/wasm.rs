use anyhow::{anyhow, Result};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::Mutex; 
use wasmtime::*;
use wasmtime_wasi::preview1::{WasiP1Ctx}; 
use wasmtime_wasi::WasiCtxBuilder;
use async_trait::async_trait;
use serde_json::{Value};
use crate::extension::Extension;

pub struct HostState {
    pub storage: Arc<Mutex<HashMap<String, String>>>,
    pub db: Arc<sled::Db>, 
    pub logger: Arc<dyn Fn(&str, u32) + Send + Sync>,
    pub http_allowlist: Vec<String>,
    pub env_allowlist: Vec<String>,
    pub workspace_root: Option<PathBuf>,
    pub max_body_size: usize,
    pub wasi: WasiP1Ctx, 
}

pub struct WasmExtension {
    pub name: String,
    pub instance: Instance,
    pub store: Arc<Mutex<Store<HostState>>>, 
}

#[async_trait]
impl Extension for WasmExtension {
    fn name(&self) -> &str { &self.name }
    fn version(&self) -> &str { "1.8.1-export-checker" }
    fn init(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> { Ok(()) }

    async fn call(&self, func_name: &str, params: Value) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        let mut store = self.store.lock().await;
        
        let func = self.instance.get_func(&mut *store, func_name).ok_or(anyhow!("WASM Function '{}' not found", func_name))?;
        let alloc = self.instance.get_func(&mut *store, "alloc").ok_or(anyhow!("'alloc' not found"))?;
        let get_ptr = self.instance.get_func(&mut *store, "get_output_ptr").ok_or(anyhow!("'get_output_ptr' not found"))?;
        let memory = self.instance.get_memory(&mut *store, "memory").ok_or("No 'memory' export found")?;

        let param_str = params.to_string();
        let param_bytes = param_str.as_bytes().to_vec();
        println!("🔍 [DEBUG] Calling WASM: {}", func_name);

        let mut alloc_res = [Val::I32(0)];
        alloc.call(&mut *store, &[Val::I32(param_bytes.len() as i32)], &mut alloc_res)?;
        let input_ptr = alloc_res[0].i32().unwrap() as u32;

        memory.write(&mut *store, input_ptr as usize, &param_bytes)?;

        let mut exec_res = [Val::I32(0)];
        func.call(&mut *store, &[Val::I32(input_ptr as i32), Val::I32(param_bytes.len() as i32)], &mut exec_res)?;
        let output_len = exec_res[0].i32().unwrap_or(0);
        
        if output_len <= 0 { 
            return Err(anyhow!("WASM Execution Logic Error. Code: {}", output_len).into()); 
        }

        let mut ptr_res = [Val::I32(0)];
        get_ptr.call(&mut *store, &[], &mut ptr_res)?;
        let output_ptr = ptr_res[0].i32().unwrap() as u32;

        let mut output_buffer = vec![0u8; output_len as usize];
        memory.read(&mut *store, output_ptr as usize, &mut output_buffer)?;
        
        Ok(serde_json::from_slice(&output_buffer)?)
    }
}

pub fn read_string_from_memory<T>(caller: &mut Caller<'_, T>, ptr: u32, len: u32) -> Result<String> {
    let mem = caller.get_export("memory").and_then(|e| e.into_memory()).ok_or_else(|| anyhow!("No memory"))?;
    let data = mem.data(&*caller);
    let slice = data.get(ptr as usize..(ptr + len) as usize).ok_or_else(|| anyhow!("Out of bounds"))?;
    Ok(String::from_utf8_lossy(slice).to_string())
}

pub async fn load_wasm_plugin(
    path: &Path,
    name: String,
    _version: String,
    _memory_limit: usize,
    fuel_limit: u64,
    logger: Arc<dyn Fn(&str, u32) + Send + Sync>,
    http_allowlist: Vec<String>,
    env_allowlist: Vec<String>,
    workspace_root: Option<PathBuf>,
    max_body_size: usize,
) -> Result<Box<dyn Extension>> {
    let mut config = Config::new();
    config.async_support(false).consume_fuel(true);

    let engine = Engine::new(&config)?;
    let module = Module::from_file(&engine, path)?;
    let mut linker: Linker<HostState> = Linker::new(&engine);
    
    // 手动注册同步 WASI 接口
    linker.func_wrap("wasi_snapshot_preview1", "environ_get", |_: Caller<'_, HostState>, _: i32, _: i32| -> i32 { 0 })?;
    linker.func_wrap("wasi_snapshot_preview1", "environ_sizes_get", |_: Caller<'_, HostState>, _: i32, _: i32| -> i32 { 0 })?;
    linker.func_wrap::<_, ()>("wasi_snapshot_preview1", "proc_exit", |_: Caller<'_, HostState>, _: i32| { std::process::exit(0); })?;
    linker.func_wrap("wasi_snapshot_preview1", "fd_close", |_: Caller<'_, HostState>, _: i32| -> i32 { 0 })?;
    linker.func_wrap("wasi_snapshot_preview1", "fd_seek", |_: Caller<'_, HostState>, _: i32, _: i64, _: i32, _: i32| -> i32 { 0 })?;
    linker.func_wrap("wasi_snapshot_preview1", "fd_write", |_: Caller<'_, HostState>, _: i32, _: i32, _: i32, _: i32| -> i32 { 0 })?;

    linker.func_wrap("env", "host_log", |mut caller: Caller<'_, HostState>, level: u32, ptr: u32, len: u32| {
        if let Ok(msg) = read_string_from_memory(&mut caller, ptr, len) {
            println!("💬 [PLUGIN LOG] L{}: {}", level, msg);
        }
    })?;

    let db = Arc::new(sled::open(".agent_db")?);
    let state = HostState {
        storage: Arc::new(Mutex::new(HashMap::new())),
        db,
        logger,
        http_allowlist,
        env_allowlist,
        workspace_root,
        max_body_size,
        wasi: WasiCtxBuilder::new().build_p1(),
    };

    let mut store = Store::new(&engine, state);
    store.set_fuel(fuel_limit)?; 
    
    let instance = linker.instantiate(&mut store, &module)?;

    // 🚀 修复后的导出项打印逻辑
    println!("📦 WASM Exports for '{}':", name);
    for export in instance.exports(&mut store) {
        let name = export.name().to_string();
        let kind = if export.into_func().is_some() { "Func" } else { "Other" };
        println!("  - [{}] {}", kind, name);
    }
    
    Ok(Box::new(WasmExtension { name, instance, store: Arc::new(Mutex::new(store)) }))
}