//! HTTP 相关宿主函数
//!
//! 提供 HTTP 客户端功能，支持 GET/POST/PUT/DELETE/PATCH 等方法，以及联网搜索

use wasmtime::{Linker, Caller};
use crate::extension::wasm::{HostState, read_string_from_memory, write_bytes_to_memory};
use anyhow::Result;
use reqwest::blocking::Client;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::time::Duration;
use base64::Engine;  // 必须导入 Engine trait 才能调用 encode

/// 注册所有 HTTP 相关函数
pub fn register_http(linker: &mut Linker<HostState>) -> Result<()> {
    // ===================== http_post (保留原有) =====================
    linker.func_wrap("env", "http_post", |mut caller: Caller<'_, HostState>,
                     url_ptr: u32, url_len: u32,
                     body_ptr: u32, body_len: u32,
                     result_ptr: u32| -> Result<i32, wasmtime::Error> {
        
        let mem = match caller.get_export("memory").and_then(|e| e.into_memory()) {
            Some(mem) => mem,
            None => return Err(wasmtime::Error::msg("no memory")),
        };

        let url = match read_string_from_memory(&mem, &caller, url_ptr, url_len) {
            Ok(s) => s,
            Err(_) => return Err(wasmtime::Error::msg("invalid URL")),
        };

        let body = match read_string_from_memory(&mem, &caller, body_ptr, body_len) {
            Ok(s) => s,
            Err(_) => return Err(wasmtime::Error::msg("invalid body")),
        };

        let host_state = caller.data();

        let domain = url.split('/').nth(2).unwrap_or("");
        if !host_state.http_allowlist.iter().any(|allowed| domain.starts_with(allowed)) {
            eprintln!("Domain not allowed: {}", domain);
            return Err(wasmtime::Error::msg("domain not allowed"));
        }

        const MAX_BODY_SIZE: usize = 1024 * 1024;
        if body.len() > MAX_BODY_SIZE {
            eprintln!("Body too large: {} > {}", body.len(), MAX_BODY_SIZE);
            return Err(wasmtime::Error::msg("body too large"));
        }

        let client = Client::new();
        let mut response = match client.post(&url).body(body).send() {
            Ok(resp) => resp,
            Err(e) => {
                eprintln!("HTTP POST failed: {}", e);
                return Err(wasmtime::Error::msg("request failed"));
            }
        };

        let mut response_bytes = Vec::new();
        if let Err(e) = response.copy_to(&mut response_bytes) {
            eprintln!("Failed to read response: {}", e);
            return Err(wasmtime::Error::msg("read response failed"));
        }

        if response_bytes.len() > host_state.max_body_size {
            eprintln!("Response too large: {} > {}", response_bytes.len(), host_state.max_body_size);
            return Err(wasmtime::Error::msg("response too large"));
        }

        write_bytes_to_memory(&mut caller, result_ptr, &response_bytes)
            .map_err(|e| wasmtime::Error::msg(e.to_string()))?;

        Ok(0)
    })?;

    // ===================== host_web_search (模拟结果，保留) =====================
    linker.func_wrap("env", "host_web_search", |mut caller: Caller<'_, HostState>,
                     query_ptr: u32, query_len: u32,
                     result_ptr: u32| -> Result<i32, wasmtime::Error> {
        let mem = match caller.get_export("memory").and_then(|e| e.into_memory()) {
            Some(mem) => mem,
            None => return Err(wasmtime::Error::msg("no memory")),
        };

        // 可选：读取查询（虽不用于模拟，但保留）
        let _query = match read_string_from_memory(&mem, &caller, query_ptr, query_len) {
            Ok(s) => s,
            Err(_) => return Err(wasmtime::Error::msg("invalid query")),
        };

        let host_state = caller.data();

        // 模拟搜索结果
        let mock_results = json!({
            "results": [
                { "title": "Rust WASM", "url": "https://example.com/1", "snippet": "Rust and WebAssembly are a great combination." },
                { "title": "Another Result", "url": "https://example.com/2", "snippet": "Mock result for testing." }
            ]
        }).to_string();
        let body = mock_results.into_bytes();

        if body.len() > host_state.max_body_size {
            return Err(wasmtime::Error::msg("response too large"));
        }

        write_bytes_to_memory(&mut caller, result_ptr, &body)
            .map_err(|e| wasmtime::Error::msg(e.to_string()))?;

        Ok(0)
    })?;

    // ===================== 新增：通用 HTTP 请求 (host_http_request) =====================
    linker.func_wrap("env", "host_http_request", 
        |mut caller: Caller<'_, HostState>,
         input_ptr: u32,
         input_len: u32,
         output_ptr: u32|
         -> Result<i32, wasmtime::Error> {

            // 1. 读取输入 JSON 字符串
            let mem = match caller.get_export("memory").and_then(|e| e.into_memory()) {
                Some(mem) => mem,
                None => return Err(wasmtime::Error::msg("no memory")),
            };
            let input_str = match read_string_from_memory(&mem, &caller, input_ptr, input_len) {
                Ok(s) => s,
                Err(_) => {
                    eprintln!("host_http_request: failed to read input string");
                    return Err(wasmtime::Error::msg("invalid input"));
                }
            };

            // 2. 解析 JSON 参数
            let params: Value = match serde_json::from_str(&input_str) {
                Ok(v) => v,
                Err(e) => {
                    eprintln!("host_http_request: invalid JSON: {}", e);
                    return Err(wasmtime::Error::msg("invalid JSON"));
                }
            };

            let method = params["method"].as_str().unwrap_or("GET").to_uppercase();
            let url = params["url"].as_str().ok_or_else(|| {
                wasmtime::Error::msg("missing url")
            })?;
            let headers = params.get("headers").and_then(|h| h.as_object())
                .map(|obj| {
                    obj.iter()
                        .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                        .collect::<HashMap<String, String>>()
                })
                .unwrap_or_default();
            let body = params.get("body").and_then(|b| b.as_str())
                .map(|s| s.as_bytes().to_vec())
                .unwrap_or_default();
            let proxy = params.get("proxy").and_then(|p| p.as_str())
                .map(|s| s.to_string());

            // 3. 安全校验：域名白名单
            let host_state = caller.data();
            let domain = url.split('/').nth(2).unwrap_or("");
            if !host_state.http_allowlist.iter().any(|allowed| domain.starts_with(allowed)) {
                eprintln!("host_http_request: domain not allowed: {}", domain);
                return Err(wasmtime::Error::msg("domain not allowed"));
            }

            // 4. 构建 HTTP 客户端
            let mut client_builder = Client::builder()
                .timeout(Duration::from_secs(30))
                .redirect(reqwest::redirect::Policy::limited(5));
            if let Some(proxy_url) = proxy {
                if let Ok(proxy) = reqwest::Proxy::all(&proxy_url) {
                    client_builder = client_builder.proxy(proxy);
                } else {
                    eprintln!("host_http_request: invalid proxy URL: {}", proxy_url);
                    return Err(wasmtime::Error::msg("invalid proxy"));
                }
            }
            let client = match client_builder.build() {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("host_http_request: failed to build client: {}", e);
                    return Err(wasmtime::Error::msg("client build failed"));
                }
            };

            // 5. 构建请求
            let req_method = match method.as_str() {
                "GET" => reqwest::Method::GET,
                "POST" => reqwest::Method::POST,
                "PUT" => reqwest::Method::PUT,
                "DELETE" => reqwest::Method::DELETE,
                "PATCH" => reqwest::Method::PATCH,
                _ => {
                    eprintln!("host_http_request: unsupported method: {}", method);
                    return Err(wasmtime::Error::msg("unsupported method"));
                }
            };
            let mut request = client.request(req_method, url);
            for (k, v) in headers {
                request = request.header(&k, &v);
            }
            if !body.is_empty() {
                request = request.body(body);
            }

            // 6. 执行请求
            let response = match request.send() {
                Ok(resp) => resp,
                Err(e) => {
                    eprintln!("host_http_request: request failed: {}", e);
                    return Err(wasmtime::Error::msg("request failed"));
                }
            };

            let status = response.status().as_u16();
            let response_headers = response.headers().iter()
                .map(|(k, v)| (k.as_str().to_string(), v.to_str().unwrap_or("").to_string()))
                .collect::<HashMap<String, String>>();
            let response_body = match response.bytes() {
                Ok(bytes) => bytes.to_vec(),
                Err(e) => {
                    eprintln!("host_http_request: failed to read response body: {}", e);
                    return Err(wasmtime::Error::msg("read body failed"));
                }
            };

            if response_body.len() > host_state.max_body_size {
                eprintln!("host_http_request: response too large: {} > {}", response_body.len(), host_state.max_body_size);
                return Err(wasmtime::Error::msg("response too large"));
            }

            // 7. 构造返回 JSON
            let result = json!({
                "status": status,
                "headers": response_headers,
                "body": base64::engine::general_purpose::STANDARD.encode(&response_body),
            });
            let result_str = result.to_string();
            let result_bytes = result_str.into_bytes();

            // 8. 写入输出缓冲区
            if let Err(e) = write_bytes_to_memory(&mut caller, output_ptr, &result_bytes) {
                eprintln!("host_http_request: failed to write result: {}", e);
                return Err(wasmtime::Error::msg("write result failed"));
            }

            Ok(0)
        })?;

    Ok(())
}

// ===================== workspace_delete (安全删除) =====================
pub fn register_fs(linker: &mut Linker<HostState>) -> Result<()> {
    // 注意：这里省略了原有的注册代码（如 workspace_write 等），仅展示新增部分。
    // 实际使用时，应将此函数放在 fs.rs 中，并包含原有注册内容。
    // 为避免破坏原有功能，请确保原有的 fs 注册内容也保留在此处（如果放在 http.rs 中）。
    // 以下仅包含 workspace_delete 和 workspace_mkdir 的注册，原有内容请自行添加。

    // 新增 workspace_delete 函数
    linker.func_wrap("env", "workspace_delete", 
        |mut caller: Caller<'_, HostState>,
         path_ptr: u32,
         path_len: u32,
         confirm_ptr: u32,
         confirm_len: u32,
         result_ptr: u32|
         -> Result<i32, wasmtime::Error> {

            let mem = match caller.get_export("memory").and_then(|e| e.into_memory()) {
                Some(mem) => mem,
                None => return Err(wasmtime::Error::msg("no memory")),
            };

            let path = match read_string_from_memory(&mem, &caller, path_ptr, path_len) {
                Ok(s) => s,
                Err(_) => return Err(wasmtime::Error::msg("invalid path")),
            };
            let confirm = match read_string_from_memory(&mem, &caller, confirm_ptr, confirm_len) {
                Ok(s) => s,
                Err(_) => return Err(wasmtime::Error::msg("invalid confirm")),
            };

            let host_state = caller.data();
            let workspace_root = match &host_state.workspace_root {
                Some(root) => root,
                None => return Err(wasmtime::Error::msg("workspace not available")),
            };

            // 安全检查：路径不能包含 .. 或绝对路径
            if path.starts_with('/') || path.starts_with("..") || path.contains("../") {
                eprintln!("Invalid path: {}", path);
                return Err(wasmtime::Error::msg("path not allowed"));
            }

            let full_path = workspace_root.join(&path);

            // 确保路径在工作区内（规范化后检查）
            let canonical = match std::fs::canonicalize(&full_path) {
                Ok(c) => c,
                Err(_) => {
                    // 如果文件不存在，我们仍然允许删除（返回成功）
                    // 但这里为了安全，先检查是否存在
                    if !full_path.exists() {
                        // 文件不存在，视为成功
                        let success_msg = serde_json::json!({ "success": true, "message": "path does not exist" }).to_string();
                        let bytes = success_msg.into_bytes();
                        write_bytes_to_memory(&mut caller, result_ptr, &bytes)
                            .map_err(|e| wasmtime::Error::msg(e.to_string()))?;
                        return Ok(0);
                    } else {
                        return Err(wasmtime::Error::msg("cannot canonicalize path"));
                    }
                }
            };
            if !canonical.starts_with(workspace_root) {
                eprintln!("Path escapes workspace: {}", canonical.display());
                return Err(wasmtime::Error::msg("path outside workspace"));
            }

            // 二次确认：必须为 "yes" 或 "true"
            if confirm.to_lowercase() != "yes" && confirm.to_lowercase() != "true" {
                eprintln!("Delete not confirmed: confirm={}", confirm);
                return Err(wasmtime::Error::msg("delete not confirmed"));
            }

            // 执行删除
            let result = if full_path.is_dir() {
                std::fs::remove_dir_all(&full_path)
            } else {
                std::fs::remove_file(&full_path)
            };

            match result {
                Ok(_) => {
                    let success_msg = serde_json::json!({ "success": true, "message": "deleted" }).to_string();
                    let bytes = success_msg.into_bytes();
                    write_bytes_to_memory(&mut caller, result_ptr, &bytes)
                        .map_err(|e| wasmtime::Error::msg(e.to_string()))?;
                    Ok(0)
                }
                Err(e) => {
                    eprintln!("Delete failed: {}", e);
                    Err(wasmtime::Error::msg("delete failed"))
                }
            }
        })?;

    // ===================== workspace_mkdir =====================
    linker.func_wrap("env", "workspace_mkdir", 
        |mut caller: Caller<'_, HostState>,
         path_ptr: u32,
         path_len: u32,
         recursive_ptr: u32,
         recursive_len: u32,
         result_ptr: u32|
         -> Result<i32, wasmtime::Error> {

            let mem = match caller.get_export("memory").and_then(|e| e.into_memory()) {
                Some(mem) => mem,
                None => return Err(wasmtime::Error::msg("no memory")),
            };

            let path = match read_string_from_memory(&mem, &caller, path_ptr, path_len) {
                Ok(s) => s,
                Err(_) => return Err(wasmtime::Error::msg("invalid path")),
            };
            let recursive = match read_string_from_memory(&mem, &caller, recursive_ptr, recursive_len) {
                Ok(s) => s,
                Err(_) => return Err(wasmtime::Error::msg("invalid recursive flag")),
            };

            let host_state = caller.data();
            let workspace_root = match &host_state.workspace_root {
                Some(root) => root,
                None => return Err(wasmtime::Error::msg("workspace not available")),
            };

            if path.starts_with('/') || path.starts_with("..") || path.contains("../") {
                eprintln!("Invalid path: {}", path);
                return Err(wasmtime::Error::msg("path not allowed"));
            }

            let full_path = workspace_root.join(&path);

            // 确保父目录在工作区内
            if let Some(parent) = full_path.parent() {
                if parent != workspace_root {
                    let parent_canonical = match std::fs::canonicalize(parent) {
                        Ok(p) => p,
                        Err(_) => parent.to_path_buf(),
                    };
                    if !parent_canonical.starts_with(workspace_root) {
                        eprintln!("Parent directory outside workspace: {}", parent_canonical.display());
                        return Err(wasmtime::Error::msg("parent directory outside workspace"));
                    }
                }
            }

            let recursive_flag = recursive.to_lowercase() == "true" || recursive.to_lowercase() == "yes";
            let result = if recursive_flag {
                std::fs::create_dir_all(&full_path)
            } else {
                std::fs::create_dir(&full_path)
            };

            match result {
                Ok(_) => {
                    let success_msg = serde_json::json!({ "success": true, "message": "created" }).to_string();
                    let bytes = success_msg.into_bytes();
                    write_bytes_to_memory(&mut caller, result_ptr, &bytes)
                        .map_err(|e| wasmtime::Error::msg(e.to_string()))?;
                    Ok(0)
                }
                Err(e) => {
                    eprintln!("Mkdir failed: {}", e);
                    Err(wasmtime::Error::msg("mkdir failed"))
                }
            }
        })?;

    Ok(())
}