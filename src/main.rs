mod attestation;
mod audit;
mod extension;
mod governance;
mod identity;
mod ingress;
mod market;
mod observability;
mod ownership;
mod payment;
mod policy;
mod replication;
mod resource;
mod router;
mod runtime;
mod sandbox;
mod storage;
mod verification;
mod workflow;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 初始化可观测性（添加这一行）
    observability::init_observability()?;

    println!("🚀 Starting agent-core ingress server...");
    ingress::start_server().await?;
    Ok(())
}