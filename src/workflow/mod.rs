use std::collections::{HashMap, VecDeque};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use anyhow::Result;
use petgraph::graph::DiGraph;
use petgraph::Direction;
use serde::{Deserialize, Serialize};
use tokio::task::JoinSet;

use crate::extension::Extension;

// ================= 意图定义 =================
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Intent {
    Transfer { to: String, amount: u64, asset: String },
    Swap { from: String, to: String, amount: u64 },
    Stake { pool: String, amount: u64, lock_period: Option<u64> },

    EchoText { text: String },
    HttpPost { url: String, body: Option<String> },
    WorkspaceRead { path: String },
    WorkspaceDelete { path: String, confirm: String },
    PermissionCheck { resource: String, action: String },
    CostTracker { tokens: u64, cost_cents: u64 },
    WebSearch { query: String },

    BrowserOpen { url: String },
    BrowserScreenshot,
}

// ================= 任务定义 =================
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: String,
    pub name: String,
    pub input: Vec<u8>,
    pub dependencies: Vec<String>,
}

// ================= 任务状态 =================
#[derive(Debug, Clone, PartialEq)]
pub enum TaskStatus {
    Pending,
    Running,
    Completed,
    Failed(String),
}

// ================= 工作流编译器 =================
pub trait WorkflowCompiler: Send + Sync {
    fn compile(&self, intent: Intent) -> Pin<Box<dyn Future<Output = Result<Vec<Task>>> + Send>>;
}

pub struct SimpleCompiler;

impl WorkflowCompiler for SimpleCompiler {
    fn compile(&self, intent: Intent) -> Pin<Box<dyn Future<Output = Result<Vec<Task>>> + Send>> {
        Box::pin(async move {
            let tasks = match intent {
                Intent::Transfer { to, amount, asset } => vec![
                    Task {
                        id: "1".into(),
                        name: "validate_account".into(),
                        input: format!(r#"{{"to":"{}"}}"#, to).into_bytes(),
                        dependencies: vec![],
                    },
                    Task {
                        id: "2".into(),
                        name: "execute_transfer".into(),
                        input: format!(r#"{{"amount":{},"asset":"{}"}}"#, amount, asset).into_bytes(),
                        dependencies: vec!["1".into()],
                    },
                ],
                Intent::Swap { from, to, amount } => vec![
                    Task {
                        id: "1".into(),
                        name: "check_balance".into(),
                        input: format!(r#"{{"asset":"{}","amount":{}}}"#, from, amount).into_bytes(),
                        dependencies: vec![],
                    },
                    Task {
                        id: "2".into(),
                        name: "execute_swap".into(),
                        input: format!(r#"{{"from":"{}","to":"{}","amount":{}}}"#, from, to, amount).into_bytes(),
                        dependencies: vec!["1".into()],
                    },
                ],
                Intent::Stake { pool, amount, lock_period } => {
                    let lock = lock_period.unwrap_or(0);
                    vec![
                        Task {
                            id: "1".into(),
                            name: "approve_stake".into(),
                            input: format!(r#"{{"pool":"{}","amount":{}}}"#, pool, amount).into_bytes(),
                            dependencies: vec![],
                        },
                        Task {
                            id: "2".into(),
                            name: "stake_tokens".into(),
                            input: format!(r#"{{"pool":"{}","amount":{},"lock_period":{}}}"#, pool, amount, lock).into_bytes(),
                            dependencies: vec!["1".into()],
                        },
                    ]
                }
                Intent::EchoText { text } => {
                    vec![Task {
                        id: "echo_1".into(),
                        name: "echo_text".into(),
                        input: serde_json::to_vec(&serde_json::json!({ "text": text }))?,
                        dependencies: vec![],
                    }]
                }
                Intent::HttpPost { url, body } => {
                    vec![Task {
                        id: "http_1".into(),
                        name: "http_post".into(),
                        input: serde_json::to_vec(&serde_json::json!({ "url": url, "body": body }))?,
                        dependencies: vec![],
                    }]
                }
                Intent::WorkspaceRead { path } => {
                    vec![Task {
                        id: "read_1".into(),
                        name: "workspace_read".into(),
                        input: serde_json::to_vec(&serde_json::json!({ "path": path }))?,
                        dependencies: vec![],
                    }]
                }
                Intent::WorkspaceDelete { path, confirm } => {
                    vec![Task {
                        id: "del_1".into(),
                        name: "workspace_delete".into(),
                        input: serde_json::to_vec(&serde_json::json!({ "path": path, "confirm": confirm }))?,
                        dependencies: vec![],
                    }]
                }
                Intent::PermissionCheck { resource, action } => {
                    vec![Task {
                        id: "perm_1".into(),
                        name: "permission_check".into(),
                        input: serde_json::to_vec(&serde_json::json!({ "resource": resource, "action": action }))?,
                        dependencies: vec![],
                    }]
                }
                Intent::CostTracker { tokens, cost_cents } => {
                    vec![Task {
                        id: "cost_1".into(),
                        name: "cost_tracker".into(),
                        input: serde_json::to_vec(&serde_json::json!({ "tokens": tokens, "cost_cents": cost_cents }))?,
                        dependencies: vec![],
                    }]
                }
                Intent::WebSearch { query } => {
                    vec![Task {
                        id: "search_1".into(),
                        name: "web_search".into(),
                        input: serde_json::to_vec(&serde_json::json!({ "query": query }))?,
                        dependencies: vec![],
                    }]
                }
                Intent::BrowserOpen { url } => {
                    vec![Task {
                        id: "browser_open_1".into(),
                        name: "browser_open".into(),
                        input: serde_json::to_vec(&serde_json::json!({ "url": url }))?,
                        dependencies: vec![],
                    }]
                }
                Intent::BrowserScreenshot => {
                    vec![Task {
                        id: "browser_screenshot_1".into(),
                        name: "browser_screenshot".into(),
                        input: serde_json::to_vec(&serde_json::json!({}))?,
                        dependencies: vec![],
                    }]
                }
            };
            Ok(tasks)
        })
    }
}

// ================= 工作流执行器 =================
pub trait WorkflowExecutor: Send + Sync {
    fn execute(&self, tasks: Vec<Task>) -> Pin<Box<dyn Future<Output = Result<Vec<String>>> + Send>>;
    fn status(&self, task_id: &str) -> Pin<Box<dyn Future<Output = Option<TaskStatus>> + Send>>;
}

pub struct ParallelExecutor {
    plugin: Arc<dyn Extension>,
}

impl ParallelExecutor {
    pub fn new(plugin: Arc<dyn Extension>) -> Self {
        Self { plugin }
    }
}

impl WorkflowExecutor for ParallelExecutor {
    fn execute(&self, tasks: Vec<Task>) -> Pin<Box<dyn Future<Output = Result<Vec<String>>> + Send>> {
        let plugin = self.plugin.clone();
        Box::pin(async move {
            let mut id_to_idx = HashMap::new();
            for (i, t) in tasks.iter().enumerate() {
                id_to_idx.insert(t.id.clone(), i);
            }

            let mut graph = DiGraph::<usize, ()>::new();
            let mut nodes = Vec::new();
            for i in 0..tasks.len() {
                nodes.push(graph.add_node(i));
            }

            for task in &tasks {
                let task_idx = id_to_idx[&task.id];
                for dep in &task.dependencies {
                    if let Some(&dep_idx) = id_to_idx.get(dep) {
                        graph.add_edge(nodes[dep_idx], nodes[task_idx], ());
                    }
                }
            }

            let mut in_degree = vec![0usize; tasks.len()];
            for i in 0..tasks.len() {
                in_degree[i] = graph.edges_directed(nodes[i], Direction::Incoming).count();
            }

            let mut queue = VecDeque::new();
            for i in 0..tasks.len() {
                if in_degree[i] == 0 {
                    queue.push_back(i);
                }
            }

            let mut results = Vec::new();

            while !queue.is_empty() {
                let mut joinset = JoinSet::new();
                let layer_size = queue.len();

                for _ in 0..layer_size {
                    let idx = queue.pop_front().unwrap();
                    let task = tasks[idx].clone();
                    let plugin_clone = plugin.clone();
                    joinset.spawn(async move {
                        eprintln!("Executing task: {} - {}", task.id, task.name);
                        let params: serde_json::Value = serde_json::from_slice(&task.input)?;
                        // 同步调用插件（注意：这里没有 .await，因为 call 是同步的）
                        let result = plugin_clone.call(&task.name, params)
                            .map_err(|e| anyhow::anyhow!(e.to_string()))?;
                        eprintln!("Task result: {}", result);
                        Ok::<usize, anyhow::Error>(idx)
                    });
                }

                while let Some(res) = joinset.join_next().await {
                    match res {
                        Ok(Ok(idx)) => {
                            results.push(tasks[idx].id.clone());
                            for neighbor in graph.neighbors_directed(nodes[idx], Direction::Outgoing) {
                                let n_idx = *graph.node_weight(neighbor).unwrap();
                                in_degree[n_idx] -= 1;
                                if in_degree[n_idx] == 0 {
                                    queue.push_back(n_idx);
                                }
                            }
                        }
                        Ok(Err(e)) => return Err(e),
                        Err(e) => return Err(anyhow::anyhow!("Task panicked: {}", e)),
                    }
                }
            }

            Ok(results)
        })
    }

    fn status(&self, _task_id: &str) -> Pin<Box<dyn Future<Output = Option<TaskStatus>> + Send>> {
        Box::pin(async { Some(TaskStatus::Completed) })
    }
}