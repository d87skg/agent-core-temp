//! --------------------------------------------------
//! Project: Agent-Core
//! Module: src/workflow/mod.rs
//! Description: 工业级 DAG 编排引擎 (含数据库查询接口)
//! --------------------------------------------------

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use backon::{ExponentialBuilder, Retryable};
use ed25519_dalek::SigningKey;
use petgraph::graph::DiGraph;
use petgraph::Direction;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sled::Db;
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::task::JoinSet;
use uuid::Uuid;

use crate::extension::Extension;

// ================= [Web4 安全核心：能力令牌] =================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityToken {
    pub fs_whitelist: HashSet<String>,
    pub allowed_methods: HashSet<String>,
    pub mem_limit_mb: u32,
}

impl CapabilityToken {
    pub fn crypto_only() -> Self {
        let mut methods = HashSet::new();
        methods.insert("crypto_sign".to_string());
        methods.insert("crypto_verify".to_string());
        Self {
            fs_whitelist: HashSet::new(),
            allowed_methods: methods,
            mem_limit_mb: 128,
        }
    }

    pub fn fs_only(paths: Vec<String>) -> Self {
        let mut methods = HashSet::new();
        methods.insert("workspace_mkdir".to_string());
        methods.insert("workspace_read".to_string());
        Self {
            fs_whitelist: paths.into_iter().collect(),
            allowed_methods: methods,
            mem_limit_mb: 64,
        }
    }
}

// ================= [意图与任务定义] =================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Intent {
    Transfer { to: String, amount: u64, asset: String },
    EchoText { text: String },
    WorkspaceMkdir { path: String, recursive: bool },
    CryptoSign { private_key_hex: String, message: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: String,
    pub name: String,
    pub input: Vec<u8>,
    pub dependencies: Vec<String>,
    pub token: Option<CapabilityToken>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TaskStatus {
    Pending,
    Running,
    Completed,
    Failed(String),
}

// ================= [执行器接口定义] =================

#[async_trait]
pub trait WorkflowExecutor: Send + Sync {
    async fn execute(&self, tasks: Vec<Task>) -> Result<Vec<String>>;
    async fn status(&self, task_id: &str) -> Option<TaskStatus>;
}

pub struct ParallelExecutor {
    plugin: Arc<dyn Extension>,
    pub(crate) results_cache: Arc<Mutex<HashMap<String, Value>>>,
    pub(crate) db: Db,
    _signing_key: SigningKey,
}

impl ParallelExecutor {
    pub fn new(plugin: Arc<dyn Extension>, db_path: &str) -> Result<Self> {
        let db = sled::open(db_path)?;
        
        let key_path = std::path::Path::new(db_path).join("identity.key");
        let signing_key = if key_path.exists() {
            let key_bytes = std::fs::read(&key_path)?;
            let array: [u8; 32] = key_bytes.try_into().map_err(|_| anyhow!("Invalid identity key file"))?;
            SigningKey::from_bytes(&array)
        } else {
            let mut key_bytes = [0u8; 32];
            rand::thread_rng().fill_bytes(&mut key_bytes);
            let key = SigningKey::from_bytes(&key_bytes);
            if !std::path::Path::new(db_path).exists() {
                std::fs::create_dir_all(db_path)?;
            }
            std::fs::write(&key_path, key.to_bytes())?;
            key
        };

        let mut initial_cache = HashMap::new();
        for item in db.iter() {
            let (k, v) = item.map_err(|e| anyhow!("Database read error: {}", e))?;
            if let Ok(task_id) = String::from_utf8(k.to_vec()) {
                if let Ok(result) = serde_json::from_slice(&v) {
                    initial_cache.insert(task_id, result);
                }
            }
        }

        Ok(Self { 
            plugin,
            results_cache: Arc::new(Mutex::new(initial_cache)),
            db,
            _signing_key: signing_key,
        })
    }

    /// 🚀 核心新增：查询数据库结果
    pub async fn get_result(&self, id: &str) -> Result<Option<Vec<u8>>> {
        let result = self.db.get(id.as_bytes())?;
        Ok(result.map(|ivec| ivec.to_vec()))
    }

    fn resolve_placeholders(val: &mut Value, cache: &HashMap<String, Value>) {
        match val {
            Value::String(s) => {
                if s.contains("{{") && s.contains("}}") {
                    let mut new_s = s.clone();
                    for (task_id, result) in cache {
                        if let Some(obj) = result.as_object() {
                            for (key, res_val) in obj {
                                let pattern = format!("{{{{ {}.{} }}}}", task_id, key);
                                let replacement = match res_val {
                                    Value::String(rs) => rs.clone(),
                                    _ => res_val.to_string(),
                                };
                                new_s = new_s.replace(&pattern, &replacement);
                            }
                        }
                    }
                    *s = new_s;
                }
            }
            Value::Object(map) => {
                for (_, v) in map.iter_mut() {
                    Self::resolve_placeholders(v, cache);
                }
            }
            Value::Array(arr) => {
                for v in arr.iter_mut() {
                    Self::resolve_placeholders(v, cache);
                }
            }
            _ => {}
        }
    }

    fn audit_privilege(task_name: &str, _params: &Value, token: &CapabilityToken) -> Result<()> {
        if !token.allowed_methods.contains(task_name) {
            return Err(anyhow!("🔥 Security Alert: Method [{}] unauthorized", task_name));
        }
        Ok(())
    }
}

#[async_trait]
impl WorkflowExecutor for ParallelExecutor {
    async fn execute(&self, tasks: Vec<Task>) -> Result<Vec<String>> {
        let plugin = self.plugin.clone();
        let cache_clone = self.results_cache.clone();
        let db_clone = self.db.clone();
        
        let mut id_to_idx = HashMap::new();
        for (i, t) in tasks.iter().enumerate() { id_to_idx.insert(t.id.clone(), i); }

        let mut graph = DiGraph::<usize, ()>::new();
        let nodes: Vec<_> = (0..tasks.len()).map(|i| graph.add_node(i)).collect();

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
        for i in 0..tasks.len() { if in_degree[i] == 0 { queue.push_back(i); } }

        let mut final_results_ids = Vec::new();

        while !queue.is_empty() {
            let mut joinset = JoinSet::new();
            let layer_size = queue.len();

            for _ in 0..layer_size {
                let idx = queue.pop_front().unwrap();
                let task = tasks[idx].clone();
                let plugin_clone = plugin.clone();
                let task_cache = cache_clone.clone();
                let db = db_clone.clone();
                
                joinset.spawn(async move {
                    {
                        let cache = task_cache.lock().await;
                        if cache.contains_key(&task.id) {
                            return Ok::<usize, anyhow::Error>(idx);
                        }
                    }

                    let mut params: Value = serde_json::from_slice(&task.input)?;
                    {
                        let cache = task_cache.lock().await;
                        Self::resolve_placeholders(&mut params, &cache);
                    }

                    if let Some(token) = &task.token {
                        Self::audit_privilege(&task.name, &params, token)?;
                    }

                    let retry_strategy = ExponentialBuilder::default()
                        .with_max_times(3)
                        .with_min_delay(Duration::from_millis(500));

                    let task_name = task.name.clone();
                    let result = (|| async {
                        plugin_clone.call(&task_name, params.clone()).await
                            .map_err(|e| anyhow!("Extension Call Failed: {:?}", e))
                    })
                    .retry(&retry_strategy)
                    .await?; 

                    db.insert(task.id.as_bytes(), serde_json::to_vec(&result)?)?;
                    db.flush()?; 

                    {
                        let mut cache = task_cache.lock().await;
                        cache.insert(task.id.clone(), result);
                    }

                    Ok::<usize, anyhow::Error>(idx)
                });
            }

            while let Some(res) = joinset.join_next().await {
                match res {
                    Ok(Ok(idx)) => {
                        final_results_ids.push(tasks[idx].id.clone());
                        for neighbor in graph.neighbors_directed(nodes[idx], Direction::Outgoing) {
                            let n_idx = *graph.node_weight(neighbor).unwrap();
                            in_degree[n_idx] -= 1;
                            if in_degree[n_idx] == 0 { queue.push_back(n_idx); }
                        }
                    }
                    Ok(Err(e)) => return Err(anyhow!("Workflow Execution Error: {}", e)),
                    Err(e) => return Err(anyhow!("Task Panic: {:?}", e)),
                }
            }
        }
        Ok(final_results_ids)
    }

    async fn status(&self, _task_id: &str) -> Option<TaskStatus> {
        Some(TaskStatus::Completed)
    }
}