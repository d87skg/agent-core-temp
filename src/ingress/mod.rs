use crate::runtime::{RuntimeManager, SimpleRuntime};
use crate::workflow::{Intent, SimpleCompiler, WorkflowCompiler};
use anyhow::Result;
use axum::{extract::State, http::StatusCode, response::IntoResponse, routing::post, Json, Router};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::net::TcpListener;

// ---------- 请求/响应结构体 ----------

#[derive(Debug, Deserialize)]
pub struct RunRequest {
    intent: IntentPayload,
}

#[derive(Debug, Deserialize)]
pub struct IntentPayload {
    #[serde(rename = "type")]
    intent_type: String,
    to: Option<String>,
    amount: Option<u64>,
    asset: Option<String>,
    from: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct RunResponse {
    task_id: String,
    status: String,
}

// ---------- 应用状态 ----------

#[derive(Clone)]
pub struct AppState {
    compiler: Arc<SimpleCompiler>,
    runtime: Arc<SimpleRuntime>,
}

// ---------- HTTP 处理函数 ----------

async fn run_handler(
    State(state): State<AppState>,
    Json(req): Json<RunRequest>,
) -> impl IntoResponse {
    // 将请求体转换为 Intent
    let intent = match req.intent.intent_type.as_str() {
        "transfer" => Intent::Transfer {
            to: req.intent.to.unwrap_or_default(),
            amount: req.intent.amount.unwrap_or(0),
            asset: req.intent.asset.unwrap_or_default(),
        },
        _ => {
            return (
                StatusCode::BAD_REQUEST,
                Json(RunResponse {
                    task_id: "".to_string(),
                    status: "unsupported intent type".to_string(),
                }),
            );
        }
    };

    // 编译意图为任务
    let tasks = match state.compiler.compile(intent).await {
        Ok(t) => t,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(RunResponse {
                    task_id: "".to_string(),
                    status: format!("compile error: {}", e),
                }),
            );
        }
    };

    // 提交任务到运行时
    let handle = match state
        .runtime
        .submit(move || async move {
            println!("Executing {} tasks", tasks.len());
            // 这里可以实际执行 tasks，现在仅做演示
            Ok(())
        })
        .await
    {
        Ok(h) => h,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(RunResponse {
                    task_id: "".to_string(),
                    status: format!("submit error: {}", e),
                }),
            );
        }
    };

    (
        StatusCode::OK,
        Json(RunResponse {
            task_id: handle.id,
            status: "submitted".to_string(),
        }),
    )
}

// ---------- 启动服务器 ----------

pub async fn start_server() -> Result<()> {
    // 初始化运行时
    let runtime = SimpleRuntime::new(4, 100);
    let compiler = SimpleCompiler;

    let state = AppState {
        compiler: Arc::new(compiler),
        runtime: Arc::new(runtime),
    };

    // 创建路由
    let app = Router::new()
        .route("/run", post(run_handler))
        .with_state(state);

    let addr = "127.0.0.1:3000";
    let listener = TcpListener::bind(addr).await?;
    println!("🚀 Server listening on http://{}", addr);
    axum::serve(listener, app).await?;

    Ok(())
}
