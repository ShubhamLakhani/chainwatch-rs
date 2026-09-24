use crate::domain::{AppError, BlockHeader, RawLog};
use alloy_primitives::Address;
use async_trait::async_trait;

/// Port for reading canonical chain data. Implemented by the JSON-RPC client.
#[async_trait]
pub trait ChainReader: Send + Sync {
    async fn chain_id(&self) -> Result<u64, AppError>;
    async fn latest_block_number(&self) -> Result<u64, AppError>;
    async fn get_block(&self, number: u64) -> Result<Option<BlockHeader>, AppError>;
    async fn get_logs(
        &self,
        from_block: u64,
        to_block: u64,
        contract: Option<Address>,
    ) -> Result<Vec<RawLog>, AppError>;
}
