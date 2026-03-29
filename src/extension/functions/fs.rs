//! --------------------------------------------------
//! Module: src/extension/functions/fs.rs
//! Description: 文件系统 Host Functions 实现与沙箱安全检查
//! --------------------------------------------------

use wasmtime::{Linker, Caller};
use crate::extension::wasm::{HostState, read_string_from_memory, write_bytes_to_memory};
use anyhow::Result;
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};

/// 为 WASM Linker 注册所有文件操作接口
pub fn register_fs(linker: &mut Linker<HostState>) -> Result<()> {
    
    // [接口 1] workspace_read: 读取沙箱内文件
    linker.func_wrap("env", "workspace_read", |mut caller: Caller<'_, HostState>, 
                      path_ptr: u32, path_len: u32, 
                      result_ptr: u32| -> Result<i32, wasmtime::Error> {
        
        let path_str = read_string_from_memory(&mut caller, path_ptr, path_len)
            .map_err(|e| wasmtime::Error::msg(format!("Path read error: {}", e)))?;
        
        let full_path = validate_path(&path_str, caller.data())?;
        
        match fs::read(&full_path) {
            Ok(content) => {
                write_bytes_to_memory(&mut caller, result_ptr, &content)
                    .map_err(|e: anyhow::Error| wasmtime::Error::msg(e.to_string()))?;
                Ok(0) // 成功返回 0
            }
            Err(e) => {
                let err_msg = format!("FS Error: {}", e);
                let _ = write_bytes_to_memory(&mut caller, result_ptr, err_msg.as_bytes());
                Ok(1) // 失败返回 1
            }
        }
    })?;

    // [接口 2] workspace_delete: 删除文件或目录（带二次确认）
    linker.func_wrap("env", "workspace_delete", |mut caller: Caller<'_, HostState>, 
                      path_ptr: u32, path_len: u32, 
                      confirm_ptr: u32, confirm_len: u32, 
                      result_ptr: u32| -> Result<i32, wasmtime::Error> {
        
        let path_str = read_string_from_memory(&mut caller, path_ptr, path_len)
            .map_err(|e| wasmtime::Error::msg(e.to_string()))?;
        let confirm = read_string_from_memory(&mut caller, confirm_ptr, confirm_len)
            .map_err(|e| wasmtime::Error::msg(e.to_string()))?;

        if confirm != "yes" && confirm != "true" {
            return Err(wasmtime::Error::msg("Security: Deletion requires explicit 'yes' confirmation"));
        }

        let full_path = validate_path(&path_str, caller.data())?;
        
        let res = if full_path.is_dir() {
            fs::remove_dir_all(&full_path)
        } else {
            fs::remove_file(&full_path)
        };

        match res {
            Ok(_) => {
                let _ = write_bytes_to_memory(&mut caller, result_ptr, b"DELETED");
                Ok(0)
            }
            Err(e) => {
                let _ = write_bytes_to_memory(&mut caller, result_ptr, e.to_string().as_bytes());
                Ok(1)
            }
        }
    })?;

    // [接口 3] workspace_mkdir: 创建目录
    linker.func_wrap("env", "workspace_mkdir", |mut caller: Caller<'_, HostState>, 
                      path_ptr: u32, path_len: u32, 
                      result_ptr: u32| -> Result<i32, wasmtime::Error> {
        
        let path_str = read_string_from_memory(&mut caller, path_ptr, path_len)
            .map_err(|e| wasmtime::Error::msg(e.to_string()))?;
        
        let full_path = validate_path(&path_str, caller.data())?;
        
        match fs::create_dir_all(&full_path) {
            Ok(_) => {
                let _ = write_bytes_to_memory(&mut caller, result_ptr, b"OK");
                Ok(0)
            }
            Err(e) => {
                let _ = write_bytes_to_memory(&mut caller, result_ptr, e.to_string().as_bytes());
                Ok(1)
            }
        }
    })?;

    Ok(())
}

// ===================== [安全核心：路径合法性检查] =====================

/// 验证 WASM 试图访问的路径是否在授权的 workspace 范围内
fn validate_path(path_str: &str, host_state: &HostState) -> Result<PathBuf, wasmtime::Error> {
    let root = host_state.workspace_root.as_ref()
        .ok_or_else(|| wasmtime::Error::msg("Security Config Error: Workspace root not defined"))?;
    
    // 1. 拦截路径回溯攻击 (Path Traversal)
    if path_str.contains("..") || path_str.contains(":") {
        return Err(wasmtime::Error::msg("Security Violation: Path traversal or absolute path detected"));
    }
    
    // 2. 清理并拼接路径
    let relative_path = path_str.trim_start_matches(['/', '\\', '.']);
    let full_path = root.join(relative_path);
    
    // 3. 最终边界检查：确保生成的绝对路径依然在 root 内部
    let canonical_full = full_path.canonicalize().unwrap_or(full_path.clone());
    let canonical_root = root.canonicalize().unwrap_or(root.clone());
    
    if !canonical_full.starts_with(&canonical_root) {
        return Err(wasmtime::Error::msg("Security Violation: Target path is outside of workspace"));
    }
    
    Ok(full_path)
}

// ===================== [Native 原生接口实现] =====================

pub fn read(payload: Value) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
    let path_str = payload.get("path").and_then(|v| v.as_str()).ok_or("Field 'path' is missing")?;
    let path = Path::new(path_str);
    
    let content = fs::read(path)?;
    Ok(json!({
        "status": "success",
        "data_len": content.len(),
        "content_preview": String::from_utf8_lossy(&content[..content.len().min(100)]).to_string()
    }))
}