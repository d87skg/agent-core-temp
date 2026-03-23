use agent_core_temp::observability;
use agent_core_temp::ingress;
use agent_core_temp::scheduler::memory::MemoryScheduler;
use agent_core_temp::scheduler::Scheduler;
#[cfg(feature = "redis-scheduler")]
use agent_core_temp::scheduler::redis::RedisScheduler;
use agent_core_temp::idempotency::IdempotencyBackend;
use agent_core_temp::idempotency::memory::MemoryBackend;
#[cfg(feature = "sled-storage")]
use agent_core_temp::idempotency::sled::SledBackend;
use agent_core_temp::workflow::ParallelExecutor;
use agent_core_temp::extension::Extension;
use anyhow::Result;
use std::env;
use std::sync::Arc;
use tracing_subscriber;
use rand::Rng;
use serde_json::{json, Value};
use std::collections::HashMap;
use base64::Engine;
use sha2::{Sha256, Digest};
use hex;

struct MockExtension;

impl Extension for MockExtension {
    fn name(&self) -> &str {
        "mock-plugin"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn init(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    fn call(
        &self,
        method: &str,
        payload: Value,
    ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        println!("MockExtension called: method={}, payload={}", method, payload);

        match method {
            "browser_open" => {
                Ok(json!({ "result": "success", "message": "Browser opened (simulated)" }))
            }
            "browser_screenshot" => {
                let fake_png_base64 = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==";
                Ok(json!({ "result": "success", "data": fake_png_base64 }))
            }
            "echo_text" => {
                let text = payload.get("text")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                Ok(json!({ "result": "success", "echo": text }))
            }
            "web_search" => {
                let _query = payload.get("query")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                Ok(json!({
                    "results": [
                        { "title": "Rust WASM", "url": "https://example.com", "snippet": "Mock result for web_search" }
                    ]
                }))
            }
            "http_request" => {
                // 提取参数
                let method_str = payload.get("method")
                    .and_then(|v| v.as_str())
                    .unwrap_or("GET")
                    .to_uppercase();
                let url = payload.get("url")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow::anyhow!("missing url"))?
                    .to_string();
                let headers = payload.get("headers")
                    .and_then(|h| h.as_object())
                    .map(|obj| {
                        obj.iter()
                            .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                            .collect::<HashMap<String, String>>()
                    })
                    .unwrap_or_default();
                let body = payload.get("body")
                    .and_then(|b| b.as_str())
                    .map(|s| s.as_bytes().to_vec())
                    .unwrap_or_default();
                let proxy = payload.get("proxy")
                    .and_then(|p| p.as_str())
                    .map(|s| s.to_string());

                // 使用独立线程执行同步 HTTP 请求，避免嵌套运行时
                let handle = std::thread::spawn(move || -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
                    // 构建客户端（支持代理）
                    let mut client_builder = reqwest::blocking::Client::builder()
                        .timeout(std::time::Duration::from_secs(30))
                        .redirect(reqwest::redirect::Policy::limited(5));
                    if let Some(proxy_url) = proxy {
                        if let Ok(proxy) = reqwest::Proxy::all(&proxy_url) {
                            client_builder = client_builder.proxy(proxy);
                        } else {
                            return Err(anyhow::anyhow!("invalid proxy URL").into());
                        }
                    }
                    let client = client_builder.build()?;

                    // 构建请求
                    let req_method = match method_str.as_str() {
                        "GET" => reqwest::Method::GET,
                        "POST" => reqwest::Method::POST,
                        "PUT" => reqwest::Method::PUT,
                        "DELETE" => reqwest::Method::DELETE,
                        "PATCH" => reqwest::Method::PATCH,
                        _ => return Err(anyhow::anyhow!("unsupported method").into()),
                    };
                    let mut request = client.request(req_method, &url);
                    for (k, v) in headers {
                        request = request.header(&k, &v);
                    }
                    if !body.is_empty() {
                        request = request.body(body);
                    }

                    // 发送请求
                    let response = request.send()?;
                    let status = response.status().as_u16();
                    let response_headers = response.headers().iter()
                        .map(|(k, v)| (k.as_str().to_string(), v.to_str().unwrap_or("").to_string()))
                        .collect::<HashMap<String, String>>();
                    let response_body = response.bytes()?.to_vec();

                    // 构造返回 JSON
                    let result_json = json!({
                        "status": status,
                        "headers": response_headers,
                        "body": base64::engine::general_purpose::STANDARD.encode(&response_body),
                    });
                    Ok(result_json)
                });

                // 等待线程完成并获取结果
                match handle.join() {
                    Ok(Ok(result)) => Ok(result),
                    Ok(Err(e)) => Err(e),
                    Err(_) => Err(anyhow::anyhow!("HTTP request thread panicked").into()),
                }
            }
            "workspace_delete" => {
                let path = payload.get("path").and_then(|v| v.as_str()).unwrap_or("");
                let confirm = payload.get("confirm").and_then(|v| v.as_str()).unwrap_or("");
                if confirm == "yes" || confirm == "true" {
                    // 模拟删除（实际可调用 std::fs 但需注意权限）
                    Ok(json!({ "success": true, "message": format!("Deleted (simulated): {}", path) }))
                } else {
                    Ok(json!({ "success": false, "message": "Delete not confirmed" }))
                }
            }
            "workspace_mkdir" => {
                let path = payload.get("path").and_then(|v| v.as_str()).unwrap_or("");
                let recursive = payload.get("recursive").and_then(|v| v.as_bool()).unwrap_or(false);
                // 模拟创建目录
                Ok(json!({ "success": true, "message": format!("Created (simulated): {}", path), "recursive": recursive }))
            }
            "crypto_hash" => {
                let input = payload.get("input").and_then(|v| v.as_str()).unwrap_or("");
                let hash = Sha256::digest(input.as_bytes());
                let hash_hex = hex::encode(hash);
                Ok(json!({ "hash": hash_hex }))
            }
            // 新增分支
            "workspace_rename" => {
                let old_path = payload.get("old_path").and_then(|v| v.as_str()).unwrap_or("");
                let new_path = payload.get("new_path").and_then(|v| v.as_str()).unwrap_or("");
                // 模拟重命名成功
                Ok(json!({ "success": true, "message": format!("Renamed (simulated): {} -> {}", old_path, new_path) }))
            }
            "crypto_sign_ed25519" => {
                let private_key_hex = payload.get("private_key_hex").and_then(|v| v.as_str()).unwrap_or("");
                let message = payload.get("message").and_then(|v| v.as_str()).unwrap_or("");
                // 模拟签名（真实实现可使用 ring，但为了演示，返回固定签名）
                let fake_signature = "deadbeef".repeat(32);
                Ok(json!({ "signature_hex": fake_signature }))
            }
            "crypto_verify_ed25519" => {
                let public_key_hex = payload.get("public_key_hex").and_then(|v| v.as_str()).unwrap_or("");
                let message = payload.get("message").and_then(|v| v.as_str()).unwrap_or("");
                let signature_hex = payload.get("signature_hex").and_then(|v| v.as_str()).unwrap_or("");
                // 模拟验证失败
                Ok(json!({ "valid": false }))
            }
            "database_query" => {
                let connection_string = payload.get("connection_string").and_then(|v| v.as_str()).unwrap_or("");
                let sql = payload.get("sql").and_then(|v| v.as_str()).unwrap_or("");
                let params = payload.get("params").and_then(|v| v.as_array()).unwrap_or(&vec![]);
                // 模拟返回空行
                Ok(json!({ "rows": [] }))
            }
            _ => Ok(json!({ "result": "success", "method": method })),
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("info")
        .with_target(true)
        .init();

    println!("🚀 Initializing observability...");
    observability::init_observability()?;
    println!("✅ Observability initialized");

    // 1. 选择调度器后端
    let scheduler_type = env::var("SCHEDULER_TYPE").unwrap_or_else(|_| "memory".to_string());
    let scheduler: Arc<dyn Scheduler> = match scheduler_type.as_str() {
        "memory" => {
            println!("📦 Using in-memory scheduler");
            Arc::new(MemoryScheduler::new())
        }
        #[cfg(feature = "redis-scheduler")]
        "redis" => {
            let redis_url = env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string());
            println!("📦 Using Redis scheduler at {}", redis_url);
            Arc::new(RedisScheduler::new(&redis_url, "agent-stream", "agent-group", "worker-1", 3).await?)
        }
        _ => {
            anyhow::bail!("Unknown scheduler type: {}", scheduler_type);
        }
    };

    // 2. 选择幂等性存储后端
    let storage_type = env::var("IDEMPOTENCY_STORAGE").unwrap_or_else(|_| "memory".to_string());
    let _idempotency_backend: Arc<dyn IdempotencyBackend> = match storage_type.as_str() {
        "memory" => {
            println!("📦 Using in-memory idempotency backend");
            Arc::new(MemoryBackend::new(1000))
        }
        #[cfg(feature = "sled-storage")]
        "sled" => {
            let path = env::var("SLED_PATH").unwrap_or_else(|_| "./data/sled".to_string());
            println!("📦 Using Sled idempotency backend at {}", path);
            Arc::new(SledBackend::new(&path, 1000)?)
        }
        _ => {
            anyhow::bail!("Unknown idempotency storage type: {}", storage_type);
        }
    };

    // 3. 使用模拟插件
    let plugin: Box<dyn Extension> = Box::new(MockExtension);
    let shared_plugin: Arc<dyn Extension> = Arc::from(plugin);
    let executor = Arc::new(ParallelExecutor::new(shared_plugin.clone()));

    // 4. 启动 worker 循环
    let worker_scheduler = scheduler.clone();
    tokio::spawn(async move {
        loop {
            if let Some(task) = worker_scheduler.pop().await.unwrap() {
                println!("👷 Worker got task: {}", task.id);
                if rand::thread_rng().gen_bool(0.5) {
                    worker_scheduler.ack(&task.id).await.unwrap();
                    println!("✅ Task {} acked", task.id);
                } else {
                    worker_scheduler.nack(&task.id, "simulated error").await.unwrap();
                    println!("❌ Task {} nacked", task.id);
                }
            }

            if std::env::var("DISABLE_SIMULATED_DELAY").is_ok() {
                tokio::task::yield_now().await;
            } else {
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            }
        }
    });

    // 5. 启动 HTTP 服务器
    println!("🚀 Starting agent-core ingress server...");
    ingress::start_server(scheduler, executor).await?;
    Ok(())
}