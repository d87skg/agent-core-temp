mod payment;
mod identity;
mod storage;
mod verification;
mod ownership;
mod replication;
mod workflow;
mod router;
mod market;
mod sandbox;
mod governance;
mod audit;
mod policy;
mod ingress;
mod observability;
mod runtime;
mod resource;
mod extension;
mod attestation;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("🚀 Starting agent-core ingress server...");
    ingress::start_server().await?;
    Ok(())
}