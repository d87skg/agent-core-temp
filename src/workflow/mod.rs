use std::collections::{HashMap, VecDeque};
use std::future::Future;
use std::pin::Pin;

use anyhow::Result;
use petgraph::graph::DiGraph;
use petgraph::Direction;
use serde::{Deserialize, Serialize};
use tokio::task::JoinSet;

// ================= 意图定义 =================
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Intent {
    Transfer {
        to: String,
        amount: u64,
        asset: String,
    },
    Swap {
        from: String,
        to: String,
        amount: u64,
    },
    Stake {
        pool: String,
        amount: u64,
        lock_period: Option<u64>,
    },
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

// ---------------- 简单编译器 ----------------
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
<<<<<<< HEAD
                        input: format!(r#"{{"amount":{},"asset":"{}"}}"#, amount, asset)
                            .into_bytes(),
=======
                        input: format!(r#"{{"amount":{},"asset":"{}"}}"#, amount, asset).into_bytes(),
>>>>>>> 3528dad7079d2f8d60aa22b6f97e9908ab23038a
                        dependencies: vec!["1".into()],
                    },
                ],
                Intent::Swap { from, to, amount } => vec![
                    Task {
                        id: "1".into(),
                        name: "check_balance".into(),
<<<<<<< HEAD
                        input: format!(r#"{{"asset":"{}","amount":{}}}"#, from, amount)
                            .into_bytes(),
=======
                        input: format!(r#"{{"asset":"{}","amount":{}}}"#, from, amount).into_bytes(),
>>>>>>> 3528dad7079d2f8d60aa22b6f97e9908ab23038a
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
<<<<<<< HEAD
                            input: format!(r#"{{"pool":"{}","amount":{}}}"#, pool, amount)
                                .into_bytes(),
=======
                            input: format!(r#"{{"pool":"{}","amount":{}}}"#, pool, amount).into_bytes(),
>>>>>>> 3528dad7079d2f8d60aa22b6f97e9908ab23038a
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
            };
            Ok(tasks)
        })
    }
}

// ================= 工作流执行器 =================
pub trait WorkflowExecutor: Send + Sync {
<<<<<<< HEAD
    fn execute(
        &self,
        tasks: Vec<Task>,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<String>>> + Send>>;

=======
    fn execute(&self, tasks: Vec<Task>) -> Pin<Box<dyn Future<Output = Result<Vec<String>>> + Send>>;
>>>>>>> 3528dad7079d2f8d60aa22b6f97e9908ab23038a
    fn status(&self, task_id: &str) -> Pin<Box<dyn Future<Output = Option<TaskStatus>> + Send>>;
}

// ---------------- 并行执行器 ----------------
pub struct ParallelExecutor;

impl ParallelExecutor {
    async fn execute_task(task: Task) -> Result<()> {
        println!("Executing task: {} - {}", task.id, task.name);
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        Ok(())
    }
}

impl WorkflowExecutor for ParallelExecutor {
    fn execute(&self, tasks: Vec<Task>) -> Pin<Box<dyn Future<Output = Result<Vec<String>>> + Send>> {
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
                    joinset.spawn(async move {
                        ParallelExecutor::execute_task(task).await?;
                        Ok::<usize, anyhow::Error>(idx)
                    });
                }

                while let Some(res) = joinset.join_next().await {
                    match res {
                        Ok(Ok(idx)) => {
                            results.push(tasks[idx].id.clone());
<<<<<<< HEAD

                            // 下游节点入度 -1
                            for neighbor in
                                graph.neighbors_directed(nodes[idx], Direction::Outgoing)
                            {
=======
                            for neighbor in graph.neighbors_directed(nodes[idx], Direction::Outgoing) {
>>>>>>> 3528dad7079d2f8d60aa22b6f97e9908ab23038a
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
