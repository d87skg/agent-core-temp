//! 安全与审计相关宿主函数
//!
//! 提供权限检查、审计日志等功能

use wasmtime::{Linker, Caller};
use crate::extension::wasm::{HostState, read_string_from_memory, write_string_to_memory};
use anyhow::Result;
use std::time::{SystemTime, UNIX_EPOCH};
use std::sync::Mutex;
use lazy_static::lazy_static;
use serde_json::{json, Value};

// 审计日志条目
#[derive(Debug, Clone, serde::Serialize)]
struct AuditEntry {
    timestamp: u64,
    operation: String,
    resource: String,
    status: String,
    details: String,
}

// 全局审计日志存储
lazy_static! {
    static ref AUDIT_LOG: Mutex<Vec<AuditEntry>> = Mutex::new(Vec::new());
}

/// 注册所有安全相关函数
pub fn register_security(linker: &mut Linker<HostState>) -> Result<()> {
    // ===================== permission_check =====================
    linker.func_wrap("env", "permission_check", |mut caller: Caller<'_, HostState>,
                     resource_ptr: u32, resource_len: u32,
                     action_ptr: u32, action_len: u32,
                     result_ptr: u32| -> Result<i32, wasmtime::Error> {
        
        let mem = match caller.get_export("memory").and_then(|e| e.into_memory()) {
            Some(mem) => mem,
            None => return Err(wasmtime::Error::msg("no memory")),
        };

        // 读取资源名
        let resource = match read_string_from_memory(&mem, &caller, resource_ptr, resource_len) {
            Ok(s) => s,
            Err(_) => return Err(wasmtime::Error::msg("invalid resource")),
        };

        // 读取操作名
        let action = match read_string_from_memory(&mem, &caller, action_ptr, action_len) {
            Ok(s) => s,
            Err(_) => return Err(wasmtime::Error::msg("invalid action")),
        };

        let host_state = caller.data();

        // 简单的权限检查逻辑（示例）
        let allowed = match (resource.as_str(), action.as_str()) {
            ("http", "post") | ("http", "get") => !host_state.http_allowlist.is_empty(),
            ("fs", "read") | ("fs", "write") | ("fs", "delete") => host_state.workspace_root.is_some(),
            ("env", "get") => !host_state.env_allowlist.is_empty(),
            _ => true,
        };

        let result = if allowed { "allow" } else { "deny" };
        write_string_to_memory(&mut caller, result_ptr, result)
            .map_err(|e| wasmtime::Error::msg(e.to_string()))?;

        Ok(0)
    })?;

    // ===================== audit_log =====================
    linker.func_wrap("env", "audit_log", |mut caller: Caller<'_, HostState>,
                     operation_ptr: u32, operation_len: u32,
                     resource_ptr: u32, resource_len: u32,
                     status_ptr: u32, status_len: u32,
                     details_ptr: u32, details_len: u32,
                     _result_ptr: u32| -> Result<i32, wasmtime::Error> {
        
        let mem = match caller.get_export("memory").and_then(|e| e.into_memory()) {
            Some(mem) => mem,
            None => return Err(wasmtime::Error::msg("no memory")),
        };

        let operation = read_string_from_memory(&mem, &caller, operation_ptr, operation_len)
            .map_err(|_| wasmtime::Error::msg("invalid operation"))?;
        let resource = read_string_from_memory(&mem, &caller, resource_ptr, resource_len)
            .map_err(|_| wasmtime::Error::msg("invalid resource"))?;
        let status = read_string_from_memory(&mem, &caller, status_ptr, status_len)
            .map_err(|_| wasmtime::Error::msg("invalid status"))?;
        let details = read_string_from_memory(&mem, &caller, details_ptr, details_len)
            .map_err(|_| wasmtime::Error::msg("invalid details"))?;

        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        // 先打印日志（使用原始变量）
        eprintln!("[AUDIT] time={} operation={} resource={} status={} details={}",
            timestamp, operation, resource, status, details);

        // 存储日志
        let entry = AuditEntry {
            timestamp,
            operation,
            resource,
            status,
            details,
        };
        AUDIT_LOG.lock().unwrap().push(entry);

        Ok(0)
    })?;

    // ===================== audit_query =====================
    linker.func_wrap("env", "audit_query", |mut caller: Caller<'_, HostState>,
                     filter_ptr: u32, filter_len: u32,
                     result_ptr: u32| -> Result<i32, wasmtime::Error> {
        
        let mem = match caller.get_export("memory").and_then(|e| e.into_memory()) {
            Some(mem) => mem,
            None => return Err(wasmtime::Error::msg("no memory")),
        };

        // 读取过滤条件 JSON
        let filter_str = match read_string_from_memory(&mem, &caller, filter_ptr, filter_len) {
            Ok(s) => s,
            Err(_) => return Err(wasmtime::Error::msg("invalid filter")),
        };

        // 解析过滤条件
        let filter: Value = match serde_json::from_str(&filter_str) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("Invalid filter JSON: {}", e);
                return Err(wasmtime::Error::msg("invalid filter JSON"));
            }
        };

        let operation = filter.get("operation").and_then(|v| v.as_str());
        let resource = filter.get("resource").and_then(|v| v.as_str());
        let status = filter.get("status").and_then(|v| v.as_str());
        let limit = filter.get("limit").and_then(|v| v.as_u64()).unwrap_or(100) as usize;

        // 从全局存储中获取日志
        let logs = AUDIT_LOG.lock().unwrap();
        let filtered: Vec<&AuditEntry> = logs.iter()
            .filter(|entry| {
                operation.map_or(true, |op| entry.operation == op)
                    && resource.map_or(true, |res| entry.resource == res)
                    && status.map_or(true, |st| entry.status == st)
            })
            .rev()
            .take(limit)
            .collect();

        // 序列化为 JSON
        let result_json = json!(filtered);
        let result_str = result_json.to_string();
        let result_bytes = result_str.into_bytes();

        if result_bytes.len() > caller.data().max_body_size {
            return Err(wasmtime::Error::msg("result too large"));
        }

        // 写入内存（长度前缀格式）
        let data = mem.data_mut(&mut caller);
        let start = result_ptr as usize;
        if start + 4 + result_bytes.len() > data.len() {
            return Err(wasmtime::Error::msg("buffer too small"));
        }
        let len_bytes = (result_bytes.len() as u32).to_le_bytes();
        data[start..start+4].copy_from_slice(&len_bytes);
        data[start+4..start+4+result_bytes.len()].copy_from_slice(&result_bytes);

        Ok(0)
    })?;

    Ok(())
}