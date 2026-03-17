use crate::observability;
use crate::scheduler::Scheduler;
use crate::workflow::{Intent, SimpleCompiler};
use anyhow::Result;
use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
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
    pub intent_type: String,
    pub to: Option<String>,
    pub amount: Option<u64>,
    pub asset: Option<String>,
    #[allow(dead_code)]
    pub from: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct RunResponse {
    task_id: String,
    status: String,
}

// ---------- 应用状态 ----------
#[derive(Clone)]
pub struct AppState {
    #[allow(dead_code)]
    compiler: Arc<SimpleCompiler>,
    pub scheduler: Arc<dyn Scheduler>,
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
            observability::increment_tasks_failed();
            return (
                StatusCode::BAD_REQUEST,
                Json(RunResponse {
                    task_id: "".to_string(),
                    status: "unsupported intent type".to_string(),
                }),
            );
        }
    };

    // 编译意图为任务（这里简化：将 Intent 序列化为 payload）
    let payload = match serde_json::to_vec(&intent) {
        Ok(p) => p,
        Err(e) => {
            observability::increment_tasks_failed();
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(RunResponse {
                    task_id: "".to_string(),
                    status: format!("serialize error: {}", e),
                }),
            );
        }
    };

    // 创建调度器任务
    let task = crate::scheduler::Task::new("workflow", payload, 5, None);

    // 提交到调度器
    let task_id = match state.scheduler.submit(task).await {
        Ok(id) => id,
        Err(e) => {
            observability::increment_tasks_failed();
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(RunResponse {
                    task_id: "".to_string(),
                    status: format!("scheduler submit error: {}", e),
                }),
            );
        }
    };

    observability::increment_tasks_total();

    (
        StatusCode::OK,
        Json(RunResponse {
            task_id,
            status: "submitted".to_string(),
        }),
    )
}

// ---------- metrics 处理函数 ----------
async fn metrics_handler() -> impl IntoResponse {
    match observability::prometheus_handle() {
        Some(handle) => (StatusCode::OK, handle.render()),
        None => (StatusCode::SERVICE_UNAVAILABLE, "Prometheus not initialized".to_string()),
    }
}

// ---------- 启动服务器 ----------
pub async fn start_server(scheduler: Arc<dyn Scheduler>) -> Result<()> {
    let compiler = SimpleCompiler;

    let state = AppState {
        compiler: Arc::new(compiler),
        scheduler,
    };

    let app = Router::new()
        .route("/run", post(run_handler))
        .route("/metrics", get(metrics_handler))
        .with_state(state);

    let addr = "127.0.0.1:3000";
    let listener = TcpListener::bind(addr).await?;
    println!("🚀 Server listening on http://{}", addr);
    axum::serve(listener, app).await?;

    Ok(())
}