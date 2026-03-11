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
    println!("🚀 Starting agent-core ingress server...");
    ingress::start_server().await?;
    Ok(())
}
