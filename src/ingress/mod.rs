use crate::observability;
use crate::scheduler::RedisScheduler;
use crate::workflow::{Intent, SimpleCompiler, WorkflowCompiler};
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
    scheduler: Arc<RedisScheduler>, // 替换原来的 runtime
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
            // 无效的意图类型，记录失败指标
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

    // 编译意图为任务
    let tasks = match state.compiler.compile(intent).await {
        Ok(t) => t,
        Err(e) => {
            observability::increment_tasks_failed();
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(RunResponse {
                    task_id: "".to_string(),
                    status: format!("compile error: {}", e),
                }),
            );
        }
    };

    // 将任务列表序列化为 payload（这里使用 JSON 序列化）
    let payload = match serde_json::to_vec(&tasks) {
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

    // 提交到 Redis 调度器
    let task_id = match state.scheduler.submit("workflow", payload).await {
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

    // 成功提交任务，增加任务总数计数
    observability::increment_tasks_total();

    (
        StatusCode::OK,
        Json(RunResponse {
            task_id,
            status: "submitted".to_string(),
        }),
    )
}

/// metrics 处理函数
async fn metrics_handler() -> impl IntoResponse {
    match observability::prometheus_handle() {
        Some(handle) => (StatusCode::OK, handle.render()),
        None => (StatusCode::SERVICE_UNAVAILABLE, "Prometheus not initialized".to_string()),
    }
}

// ---------- 启动服务器 ----------

pub async fn start_server(scheduler: Arc<RedisScheduler>) -> Result<()> {
    let compiler = SimpleCompiler;

    let state = AppState {
        compiler: Arc::new(compiler),
        scheduler,
    };

    // 创建路由，添加 /metrics 和 /run 路由
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