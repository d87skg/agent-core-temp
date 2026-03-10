use std::future::Future;
use std::pin::Pin;
use std::collections::{HashMap, VecDeque};
use petgraph::graph::DiGraph;
use petgraph::algo::toposort;
use serde::{Deserialize, Serialize};
use anyhow::Result;
use tokio::task::JoinSet;

/// AI 能理解的意图语义（可扩展）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Intent {
    Transfer { to: String, amount: u64, asset: String },
    Swap { from: String, to: String, amount: u64 },
    Stake { pool: String, amount: u64, lock_period: Option<u64> },
}

/// 任务定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: String,
    pub name: String,
    pub input: Vec<u8>,
    pub dependencies: Vec<String>,
}

/// 工作流编译器：将意图编译成任务 DAG
pub trait WorkflowCompiler: Send + Sync {
    fn compile(&self, intent: Intent) -> Pin<Box<dyn Future<Output = Result<Vec<Task>, anyhow::Error>> + Send>>;
}

/// 工作流执行器接口
pub trait WorkflowExecutor: Send + Sync {
    /// 提交任务 DAG 并执行
    fn execute(&self, tasks: Vec<Task>) -> Pin<Box<dyn Future<Output = Result<Vec<String>, anyhow::Error>> + Send>>;
    
    /// 获取任务状态
    fn status(&self, task_id: &str) -> Pin<Box<dyn Future<Output = Option<TaskStatus>> + Send>>;
}

/// 任务状态
#[derive(Debug, Clone, PartialEq)]
pub enum TaskStatus {
    Pending,
    Running,
    Completed,
    Failed(String),
}

// ---------- 编译器实现 ----------

/// 一个简单的编译器，支持多种意图
pub struct SimpleCompiler;

impl WorkflowCompiler for SimpleCompiler {
    fn compile(&self, intent: Intent) -> Pin<Box<dyn Future<Output = Result<Vec<Task>, anyhow::Error>> + Send>> {
        Box::pin(async move {
            match intent {
                Intent::Transfer { to, amount, asset } => {
                    Ok(vec![
                        Task {
                            id: "1".to_string(),
                            name: "validate_account".to_string(),
                            input: format!("{{\"to\":\"{}\"}}", to).into_bytes(),
                            dependencies: vec![],
                        },
                        Task {
                            id: "2".to_string(),
                            name: "execute_transfer".to_string(),
                            input: format!("{{\"amount\":{},\"asset\":\"{}\"}}", amount, asset).into_bytes(),
                            dependencies: vec!["1".to_string()],
                        },
                    ])
                }
                Intent::Swap { from, to, amount } => {
                    Ok(vec![
                        Task {
                            id: "1".to_string(),
                            name: "check_balance".to_string(),
                            input: format!("{{\"asset\":\"{}\",\"amount\":{}}}", from, amount).into_bytes(),
                            dependencies: vec![],
                        },
                        Task {
                            id: "2".to_string(),
                            name: "execute_swap".to_string(),
                            input: format!("{{\"from\":\"{}\",\"to\":\"{}\",\"amount\":{}}}", from, to, amount).into_bytes(),
                            dependencies: vec!["1".to_string()],
                        },
                    ])
                }
                Intent::Stake { pool, amount, lock_period } => {
                    let lock = lock_period.unwrap_or(0);
                    Ok(vec![
                        Task {
                            id: "1".to_string(),
                            name: "approve_stake".to_string(),
                            input: format!("{{\"pool\":\"{}\",\"amount\":{}}}", pool, amount).into_bytes(),
                            dependencies: vec![],
                        },
                        Task {
                            id: "2".to_string(),
                            name: "stake_tokens".to_string(),
                            input: format!("{{\"pool\":\"{}\",\"amount\":{},\"lock_period\":{}}}", pool, amount, lock).into_bytes(),
                            dependencies: vec!["1".to_string()],
                        },
                    ])
                }
            }
        })
    }
}

// ---------- 并行执行器实现 ----------

/// 基于图的并行执行器
pub struct ParallelExecutor;

impl ParallelExecutor {
    /// 模拟执行单个任务（未来可以调用真实逻辑）
    async fn execute_task(task: &Task) -> Result<()> {
        println!("Executing task: {} - {}", task.id, task.name);
        // 模拟耗时
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        Ok(())
    }
}

impl WorkflowExecutor for ParallelExecutor {
    fn execute(&self, tasks: Vec<Task>) -> Pin<Box<dyn Future<Output = Result<Vec<String>, anyhow::Error>> + Send>> {
        Box::pin(async move {
            // 建立任务 ID 到索引的映射
            let mut id_to_idx: HashMap<String, usize> = HashMap::new();
            for (i, task) in tasks.iter().enumerate() {
                id_to_idx.insert(task.id.clone(), i);
            }

            // 构建有向图，节点权重存储任务索引
            let mut graph = DiGraph::<usize, ()>::new();
            let mut node_indices = Vec::with_capacity(tasks.len());
            for i in 0..tasks.len() {
                node_indices.push(graph.add_node(i));
            }

            // 添加依赖边
            for task in &tasks {
                let from_idx = id_to_idx[&task.id];
                let from_node = node_indices[from_idx];
                for dep in &task.dependencies {
                    if let Some(&to_idx) = id_to_idx.get(dep) {
                        let to_node = node_indices[to_idx];
                        graph.add_edge(from_node, to_node, ());
                    }
                }
            }

            // 计算入度（基于任务索引）
            let mut in_degree = vec![0; tasks.len()];
            for i in 0..tasks.len() {
                in_degree[i] = graph.edges_directed(node_indices[i], petgraph::Direction::Incoming).count();
            }

            let mut queue = VecDeque::new();
            for i in 0..tasks.len() {
                if in_degree[i] == 0 {
                    queue.push_back(i);
                }
            }

            let mut results = Vec::new();
            while !queue.is_empty() {
                let mut handles = JoinSet::new();
                // 当前层所有可执行的任务
                for _ in 0..queue.len() {
                    if let Some(idx) = queue.pop_front() {
                        let task = tasks[idx].clone();
                        handles.spawn(async move {
                            Self::execute_task(&task).await?;
                            Ok::<usize, anyhow::Error>(idx)
                        });
                    }
                }

                // 等待当前层所有任务完成
                while let Some(res) = handles.join_next().await {
                    match res {
                        Ok(Ok(idx)) => {
                            results.push(tasks[idx].id.clone());
                            // 更新下游节点的入度（通过节点索引找到任务索引）
                            for neighbor in graph.neighbors(node_indices[idx]) {
                                let n_idx = *graph.node_weight(neighbor).unwrap(); // 节点权重是任务索引
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
        Box::pin(async move { Some(TaskStatus::Completed) })
    }
}