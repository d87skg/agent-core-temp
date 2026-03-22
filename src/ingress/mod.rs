use crate::observability;
use crate::scheduler::Scheduler;
use crate::workflow::{Intent, SimpleCompiler, ParallelExecutor, WorkflowCompiler, WorkflowExecutor};
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
    pub from: Option<String>,
    // 新字段用于测试意图
    pub text: Option<String>,
    pub url: Option<String>,
    pub body: Option<String>,
    pub path: Option<String>,
    pub confirm: Option<String>,
    pub resource: Option<String>,
    pub action: Option<String>,
    pub tokens: Option<u64>,
    pub cost_cents: Option<u64>,
    pub query: Option<String>,  // 用于 web_search
}

#[derive(Debug, Serialize)]
pub struct RunResponse {
    pub task_ids: Vec<String>,
    pub status: String,
}

// ---------- 应用状态 ----------
#[derive(Clone)]
pub struct AppState {
    #[allow(dead_code)]
    compiler: Arc<SimpleCompiler>,
    pub scheduler: Arc<dyn Scheduler>,
    pub executor: Arc<ParallelExecutor>,
}

// ---------- 辅助函数：将请求转换为 Intent ----------
fn parse_intent(payload: &IntentPayload) -> Result<Intent> {
    match payload.intent_type.as_str() {
        "transfer" => Ok(Intent::Transfer {
            to: payload.to.clone().unwrap_or_default(),
            amount: payload.amount.unwrap_or(0),
            asset: payload.asset.clone().unwrap_or_default(),
        }),
        "echo_text" => Ok(Intent::EchoText {
            text: payload.text.clone().unwrap_or_default(),
        }),
        "http_post" => Ok(Intent::HttpPost {
            url: payload.url.clone().unwrap_or_default(),
            body: payload.body.clone(),
        }),
        "workspace_read" => Ok(Intent::WorkspaceRead {
            path: payload.path.clone().unwrap_or_default(),
        }),
        "workspace_delete" => Ok(Intent::WorkspaceDelete {
            path: payload.path.clone().unwrap_or_default(),
            confirm: payload.confirm.clone().unwrap_or_else(|| "no".to_string()),
        }),
        "permission_check" => Ok(Intent::PermissionCheck {
            resource: payload.resource.clone().unwrap_or_default(),
            action: payload.action.clone().unwrap_or_default(),
        }),
        "cost_tracker" => Ok(Intent::CostTracker {
            tokens: payload.tokens.unwrap_or(0),
            cost_cents: payload.cost_cents.unwrap_or(0),
        }),
        "web_search" => Ok(Intent::WebSearch {
            query: payload.query.clone().unwrap_or_default(),
        }),
        "browser_open" => Ok(Intent::BrowserOpen {
            url: payload.url.clone().unwrap_or_default(),
        }),
        "browser_screenshot" => Ok(Intent::BrowserScreenshot),
        _ => Err(anyhow::anyhow!("Unsupported intent type: {}", payload.intent_type)),
    }
}

// ---------- HTTP 处理函数 ----------
async fn run_handler(
    State(state): State<AppState>,
    Json(req): Json<RunRequest>,
) -> impl IntoResponse {
    let intent = match parse_intent(&req.intent) {
        Ok(i) => i,
        Err(e) => {
            observability::increment_tasks_failed();
            return (
                StatusCode::BAD_REQUEST,
                Json(RunResponse {
                    task_ids: vec![],
                    status: format!("Invalid intent: {}", e),
                }),
            );
        }
    };

    let compiler = &state.compiler;
    let tasks = match compiler.compile(intent).await {
        Ok(t) => t,
        Err(e) => {
            observability::increment_tasks_failed();
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(RunResponse {
                    task_ids: vec![],
                    status: format!("Compile error: {}", e),
                }),
            );
        }
    };

    let task_ids = match state.executor.execute(tasks).await {
        Ok(ids) => ids,
        Err(e) => {
            observability::increment_tasks_failed();
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(RunResponse {
                    task_ids: vec![],
                    status: format!("Execution error: {}", e),
                }),
            );
        }
    };

    observability::increment_tasks_total();

    (
        StatusCode::OK,
        Json(RunResponse {
            task_ids,
            status: "completed".to_string(),
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
pub async fn start_server(
    scheduler: Arc<dyn Scheduler>,
    executor: Arc<ParallelExecutor>,
) -> Result<()> {
    let compiler = SimpleCompiler;

    let state = AppState {
        compiler: Arc::new(compiler),
        scheduler,
        executor,
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