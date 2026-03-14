// src/main.rs
use agent_core_temp::observability;
use agent_core_temp::ingress;
use anyhow::Result;
use tracing_subscriber;

#[tokio::main]
async fn main() -> Result<()> {
    // 初始化日志
    tracing_subscriber::fmt()
        .with_env_filter("info")
        .with_target(true)
        .init();

    println!("🚀 Initializing observability...");
    observability::init_observability()?;
    println!("✅ Observability initialized");

    println!("🚀 Starting agent-core ingress server...");
    ingress::start_server().await?;
    Ok(())
}