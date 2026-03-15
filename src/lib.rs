// src/lib.rs
pub mod attestation;
pub mod audit;
pub mod extension;
pub mod governance;
pub mod identity;
pub mod idempotency;
pub mod ingress;
pub mod market;
pub mod observability;
pub mod ownership;
pub mod payment;
pub mod policy;
pub mod replication;
pub mod resource;
pub mod router;
pub mod runtime;
pub mod sandbox;
pub mod storage;
pub mod verification;
pub mod workflow;
pub mod scheduler;
pub mod hlc;
// src/lib.rs
pub mod error;
pub use error::{AgentError, Result};
pub mod wasm;
