use agent_core_temp::observability;
use agent_core_temp::ingress;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 初始化可观测性
    observability::init_observability()?;

    println!("🚀 Starting agent-core ingress server...");
    ingress::start_server().await?;
    Ok(())
}