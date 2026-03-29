//! --------------------------------------------------
//! Module: src/extension/functions/http.rs
//! Description: 基于 reqwest 的异步 HTTP 宿主函数实现
//! --------------------------------------------------

use wasmtime::{Linker, Caller};
use crate::extension::wasm::{HostState, read_string_from_memory, write_bytes_to_memory};
use anyhow::Result;
use serde_json::{json, Value};
use std::sync::Arc;

/// 为 WASM Linker 注册所有网络操作接口
pub fn register_http(linker: &mut Linker<HostState>) -> Result<()> {
    
    // [接口] host_http_get: 发起 GET 请求并获取响应体
    linker.func_wrap("env", "host_http_get", |mut caller: Caller<'_, HostState>, 
                      url_ptr: u32, url_len: u32, 
                      result_ptr: u32| -> Result<i32, wasmtime::Error> {
        
        // 1. 从 WASM 内存读取 URL
        let url = read_string_from_memory(&mut caller, url_ptr, url_len)
            .map_err(|e| wasmtime::Error::msg(format!("URL decode error: {}", e)))?;

        // 2. 安全检查：验证域名是否在白名单中
        let allowlist = &caller.data().http_allowlist;
        if !allowlist.is_empty() {
            let is_allowed = allowlist.iter().any(|domain| url.contains(domain));
            if !is_allowed {
                let err_msg = format!("Security Violation: URL '{}' is not in allowlist", url);
                let _ = write_bytes_to_memory(&mut caller, result_ptr, err_msg.as_bytes());
                return Ok(403); // 返回 403 禁止访问
            }
        }

        // 3. 执行异步 HTTP 请求 (桥接 Tokio)
        let handle = tokio::runtime::Handle::current();
        let fetch_result = handle.block_on(async {
            let client = reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(10))
                .build()?;
            
            let resp = client.get(&url).send().await?;
            let status = resp.status();
            
            if !status.is_success() {
                return Err(anyhow::anyhow!("HTTP Status: {}", status));
            }
            
            let body = resp.text().await?;
            Ok(body)
        });

        // 4. 处理请求结果并写回内存
        match fetch_result {
            Ok(content) => {
                // 检查内容大小是否超过宿主配置限制
                if content.len() > caller.data().max_body_size {
                    let _ = write_bytes_to_memory(&mut caller, result_ptr, b"Error: Payload too large");
                    Ok(413) 
                } else {
                    write_bytes_to_memory(&mut caller, result_ptr, content.as_bytes())
                        .map_err(|e: anyhow::Error| wasmtime::Error::msg(e.to_string()))?;
                    Ok(0) // 成功返回 0
                }
            }
            Err(e) => {
                let err_msg = format!("Fetch failed: {}", e);
                let _ = write_bytes_to_memory(&mut caller, result_ptr, err_msg.as_bytes());
                Ok(1) // 通用错误返回 1
            }
        }
    })?;

    Ok(())
}

// ===================== [Native 原生异步调用接口] =====================

/// 供宿主内部（非 WASM）直接调用的异步请求函数
pub async fn request(payload: Value, host_state: Arc<HostState>) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
    let url = payload.get("url")
        .and_then(|v| v.as_str())
        .ok_or("Field 'url' is missing in payload")?;
    
    // 原生侧同样的白名单逻辑
    if !host_state.http_allowlist.is_empty() && !host_state.http_allowlist.iter().any(|d| url.contains(d)) {
        return Err(format!("Access Denied: {}", url).into());
    }

    let client = reqwest::Client::new();
    let response = client.get(url).send().await?;
    let status = response.status();
    let text = response.text().await?;

    Ok(json!({
        "status": status.as_u16(),
        "success": status.is_success(),
        "content": text
    }))
}