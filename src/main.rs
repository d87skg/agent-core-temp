//! --------------------------------------------------
//! Project: Agent-Core
//! File: src/main.rs (最终修复版)
//! Description: 支持同步 WASM 调用与 Sled 数据库查询的 Agent 节点。
//! --------------------------------------------------

use agent_core_temp::extension::load_extension;
use agent_core_temp::workflow::{ParallelExecutor, Task, WorkflowExecutor};

use axum::{
    extract::{State, Path},
    routing::{get, post},
    Json, Router,
};
use serde_json::{json, Value};
use std::sync::Arc;
use tokio::net::TcpListener;

/// 全局应用状态
struct AppState {
    executor: Arc<ParallelExecutor>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 1. 初始化插件系统日志
    let logger = Arc::new(|msg: &str, level: u32| {
        println!("[WASM Log L{}] {}", level, msg);
    });

    // 2. 加载核心插件
    let plugin_path = "plugins/crypto_plugin.wasm";
    
    if !std::path::Path::new(plugin_path).exists() {
        println!("⚠️  [Warning] Plugin not found at {}. System may fail on calls.", plugin_path);
    }

    println!("⚙️  Loading WASM extension: {}...", plugin_path);
    let plugin = load_extension(
        plugin_path,
        "crypto_extension",
        "1.0.0",
        256,       
        1000000,   
        logger,
    ).await.expect("Failed to load extension module"); 

    // 3. 初始化并行执行引擎 (内部包含 Sled DB)
    let db_path = "./agent_db";
    let executor_inner = ParallelExecutor::new(plugin, db_path)
        .expect("Failed to initialize database at ./agent_db");
    
    let executor = Arc::new(executor_inner);

    // 4. 构建 Axum 路由状态
    let state = Arc::new(AppState {
        executor: executor.clone(),
    });

    // 5. 注册路由
    let app = Router::new()
        .route("/", get(|| async { "Agent-Core Node is Online" }))
        .route("/execute", post(handle_execute))
        .route("/result/:id", get(handle_get_result))
        .with_state(state);

    // 6. 绑定地址并启动
    let addr = "0.0.0.0:3000";
    let listener = TcpListener::bind(addr).await?;
    println!("🚀 Agent-Core Node: http://{}", addr);
    
    axum::serve(listener, app).await?;

    Ok(())
}

/// 任务执行核心 Handler
async fn handle_execute(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<Value>,
) -> Json<Value> {
    let task_id = payload["id"].as_str().unwrap_or("task_001").to_string();
    let method = payload["method"].as_str().unwrap_or("echo").to_string();
    
    let params = if payload["params"].is_null() {
        json!({})
    } else {
        payload["params"].clone()
    };

    let task = Task {
        id: task_id,
        name: method,
        input: params.to_string().into_bytes(), 
        dependencies: vec![],
        token: None,
    };

    println!("📡 [Node] Dispatching task '{}' to WASM executor...", task.id);

    match state.executor.execute(vec![task]).await {
        Ok(results) => Json(json!({
            "status": "success",
            "executed_ids": results
        })),
        Err(e) => {
            Json(json!({
                "status": "error",
                "message": format!("{}", e)
            }))
        }
    }
}

/// 🚀 修复版结果查询 Handler
async fn handle_get_result(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Json<Value> {
    // 这里调用的是 ParallelExecutor 定义的 get_result
    match state.executor.get_result(&id).await {
        Ok(Some(data_vec)) => {
            // 尝试将二进制数据解析为 JSON
            if let Ok(result_val) = serde_json::from_slice::<Value>(&data_vec) {
                Json(json!({
                    "status": "found",
                    "id": id,
                    "data": result_val
                }))
            } else {
                // 如果解析失败，则返回 Hex 编码
                Json(json!({
                    "status": "found",
                    "id": id,
                    "data": { "raw_hex": format!("{:02x?}", data_vec) }
                }))
            }
        }
        Ok(None) => Json(json!({
            "status": "not_found",
            "message": format!("No result found for ID: {}", id)
        })),
        Err(e) => Json(json!({
            "status": "error",
            "message": format!("Database error: {}", e)
        })),
    }
}