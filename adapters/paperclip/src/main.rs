use axum::{
    routing::{get, post},
    Router,
    Json,
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    response::Json as JsonResponse,
};
use serde::{Deserialize, Serialize};
use tracing::{info, error, warn};
use tracing_subscriber;
use tokio::net::TcpListener;
use tokio::time::{timeout, Duration};
use std::sync::Arc;
use tokio::sync::Mutex;

// 导入需要的模块
use agent_core_temp::runtime::executor::{ParallelExecutor, Task as CoreTask};
use agent_core_temp::idempotency::manager::UltimateIdempotencyManager;

// ==================== 请求/响应类型 ====================

#[derive(Debug, Deserialize)]
struct TaskRequest {
    task_id: String,
    task_type: String,
    payload: serde_json::Value,
    #[serde(default = "default_timeout")]
    timeout_secs: u64,
}

fn default_timeout() -> u64 { 30 }

#[derive(Debug, Serialize)]
struct TaskResponse {
    status: String,
    result: Option<serde_json::Value>,
    error: Option<String>,
    task_id: String,
    processing_time_ms: Option<u64>,
}

#[derive(Debug, Serialize)]
struct HealthResponse {
    status: String,
    version: String,
    uptime_seconds: u64,
    workers_active: usize,
    tasks_processed: u64,
}

// ==================== 应用状态 ====================

#[derive(Clone)]
struct AppState {
    executor: Arc<ParallelExecutor>,
    metrics: Arc<Metrics>,
}

struct Metrics {
    tasks_received: Arc<Mutex<u64>>,
    tasks_completed: Arc<Mutex<u64>>,
    tasks_failed: Arc<Mutex<u64>>,
    start_time: std::time::Instant,
}

impl Metrics {
    fn new() -> Self {
        Self {
            tasks_received: Arc::new(Mutex::new(0)),
            tasks_completed: Arc::new(Mutex::new(0)),
            tasks_failed: Arc::new(Mutex::new(0)),
            start_time: std::time::Instant::now(),
        }
    }

    async fn inc_received(&self) {
        let mut val = self.tasks_received.lock().await;
        *val += 1;
    }

    async fn inc_completed(&self) {
        let mut val = self.tasks_completed.lock().await;
        *val += 1;
    }

    async fn inc_failed(&self) {
        let mut val = self.tasks_failed.lock().await;
        *val += 1;
    }

    async fn get_counts(&self) -> (u64, u64, u64) {
        (
            *self.tasks_received.lock().await,
            *self.tasks_completed.lock().await,
            *self.tasks_failed.lock().await,
        )
    }
}

// ==================== 健康检查 ====================

async fn health(State(state): State<AppState>) -> JsonResponse<HealthResponse> {
    let (_received, completed, _failed) = state.metrics.get_counts().await;
    
    JsonResponse(HealthResponse {
        status: "healthy".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        uptime_seconds: state.metrics.start_time.elapsed().as_secs(),
        workers_active: 1,
        tasks_processed: completed,
    })
}

// ==================== 任务处理 ====================

async fn task_handler(
    State(state): State<AppState>,
    Json(req): Json<TaskRequest>,
) -> impl IntoResponse {
    // 记录接收到的任务
    state.metrics.inc_received().await;
    let start = std::time::Instant::now();

    info!("Received task: {} of type {}", req.task_id, req.task_type);

    // 验证输入
    if req.task_id.is_empty() {
        warn!("Task rejected: empty task_id");
        state.metrics.inc_failed().await;
        return (
            StatusCode::BAD_REQUEST,
            Json(TaskResponse {
                status: "failed".to_string(),
                result: None,
                error: Some("task_id cannot be empty".to_string()),
                task_id: req.task_id,
                processing_time_ms: None,
            }),
        );
    }

    // 构造核心任务
    let core_task = CoreTask {
        id: req.task_id.clone(),
        workflow_id: "paperclip".to_string(),
        step_id: req.task_type.clone(),
        intent: "process".to_string(),
        payload: match serde_json::to_vec(&req.payload) {
            Ok(data) => data,
            Err(e) => {
                error!("Failed to serialize payload: {}", e);
                state.metrics.inc_failed().await;
                return (
                    StatusCode::BAD_REQUEST,
                    Json(TaskResponse {
                        status: "failed".to_string(),
                        result: None,
                        error: Some(format!("Invalid payload: {}", e)),
                        task_id: req.task_id,
                        processing_time_ms: None,
                    }),
                );
            }
        },
    };

    // 执行任务（带超时控制）
    let timeout_duration = Duration::from_secs(req.timeout_secs);
    let execution_result = timeout(timeout_duration, state.executor.execute_task(core_task)).await;

    match execution_result {
        Ok(Ok(result)) => {
            // 任务成功完成
            state.metrics.inc_completed().await;
            let elapsed = start.elapsed().as_millis() as u64;
            info!("Task {} completed in {}ms", req.task_id, elapsed);

            // 将结果反序列化为 JSON
            let result_value = match serde_json::from_slice(&result.output) {
                Ok(val) => val,
                Err(_) => serde_json::json!({ "raw": result.output }),
            };

            (
                StatusCode::OK,
                Json(TaskResponse {
                    status: "completed".to_string(),
                    result: Some(result_value),
                    error: None,
                    task_id: req.task_id,
                    processing_time_ms: Some(elapsed),
                }),
            )
        }
        Ok(Err(e)) => {
            // 任务执行失败
            state.metrics.inc_failed().await;
            let elapsed = start.elapsed().as_millis() as u64;
            error!("Task {} failed after {}ms: {}", req.task_id, elapsed, e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(TaskResponse {
                    status: "failed".to_string(),
                    result: None,
                    error: Some(e.to_string()),
                    task_id: req.task_id,
                    processing_time_ms: Some(elapsed),
                }),
            )
        }
        Err(_) => {
            // 任务超时
            state.metrics.inc_failed().await;
            warn!("Task {} timed out after {}s", req.task_id, req.timeout_secs);
            (
                StatusCode::GATEWAY_TIMEOUT,
                Json(TaskResponse {
                    status: "failed".to_string(),
                    result: None,
                    error: Some(format!("timeout after {} seconds", req.timeout_secs)),
                    task_id: req.task_id,
                    processing_time_ms: None,
                }),
            )
        }
    }
}

// ==================== 优雅关机 ====================

async fn shutdown_signal() {
    tokio::signal::ctrl_c()
        .await
        .expect("failed to install CTRL+C signal handler");
    info!("Shutdown signal received, gracefully stopping...");
}

// ==================== 主函数 ====================

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 初始化日志
    tracing_subscriber::fmt()
        .with_target(true)
        .with_thread_ids(true)
        .with_file(true)
        .with_line_number(true)
        .init();

    info!("Starting Paperclip Adapter v{}", env!("CARGO_PKG_VERSION"));

    // 初始化幂等性管理器
    let idempotency_manager = Arc::new(UltimateIdempotencyManager::new());

    // 初始化 executor
    let executor = Arc::new(ParallelExecutor::new(
        "paperclip-adapter".to_string(),
        10, // 最大并发数
        idempotency_manager,
    ));

    let metrics = Arc::new(Metrics::new());
    let state = AppState { executor, metrics };

    // 构建路由
    let app = Router::new()
        .route("/health", get(health))
        .route("/task", post(task_handler))
        .with_state(state);

    // 启动服务器
    let listener = TcpListener::bind("0.0.0.0:3001").await?;
    info!("Paperclip adapter listening on http://{}", listener.local_addr()?);

    // 优雅关机
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    info!("Server stopped");
    Ok(())
}