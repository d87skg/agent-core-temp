use std::future::Future;
use std::pin::Pin;

/// 存证证明
pub struct AttestationProof {
    pub hash: String,
    pub tx_id: Option<String>,
    pub timestamp: u64,
    pub chain: String,
}

/// 批量存证结果
pub struct BatchAttestation {
    pub batch_id: String,
    pub merkle_root: String,
    pub tx_id: Option<String>,
    pub proofs: Vec<AttestationProof>,
}

pub trait AttestationProvider: Send + Sync {
    fn attest(
        &self,
        data: &[u8],
    ) -> Pin<Box<dyn Future<Output = Result<AttestationProof, anyhow::Error>> + Send>>;
    fn attest_batch(
        &self,
        data_batch: Vec<&[u8]>,
    ) -> Pin<Box<dyn Future<Output = Result<BatchAttestation, anyhow::Error>> + Send>>;
    fn verify(
        &self,
        proof: &AttestationProof,
    ) -> Pin<Box<dyn Future<Output = Result<bool, anyhow::Error>> + Send>>;
    fn generate_certificate(
        &self,
        proof: &AttestationProof,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<u8>, anyhow::Error>> + Send>>;
}
