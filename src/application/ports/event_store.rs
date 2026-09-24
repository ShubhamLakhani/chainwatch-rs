use crate::domain::{
    AppError, BlockHeader, Checkpoint, DecodedTransfer, EventListFilter, IndexedEvent,
    PersistOutcome,
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};

#[derive(Debug, Clone)]
pub struct IndexedBlockRecord {
    pub chain_id: i64,
    pub block_number: i64,
    pub block_hash: String,
    pub parent_hash: String,
    pub indexed_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct RollbackStats {
    pub deleted_events: u64,
    pub deleted_blocks: u64,
    pub ancestor_block: u64,
}

/// Port for durable event, block, and checkpoint storage.
#[async_trait]
pub trait EventStore: Send + Sync {
    async fn ping(&self) -> Result<(), AppError>;
    async fn get_checkpoint(&self, chain_id: i64) -> Result<Option<Checkpoint>, AppError>;
    async fn get_indexed_block(
        &self,
        chain_id: i64,
        block_number: u64,
    ) -> Result<Option<IndexedBlockRecord>, AppError>;
    async fn count_events(&self, chain_id: i64) -> Result<i64, AppError>;
    async fn persist_block(
        &self,
        chain_id: i64,
        header: &BlockHeader,
        events: &[DecodedTransfer],
    ) -> Result<PersistOutcome, AppError>;
    async fn rollback_after(
        &self,
        chain_id: i64,
        ancestor_block: u64,
        ancestor_hash: &str,
    ) -> Result<RollbackStats, AppError>;
    async fn reset_chain(&self, chain_id: i64) -> Result<RollbackStats, AppError>;
    async fn list_events(&self, filter: EventListFilter) -> Result<Vec<IndexedEvent>, AppError>;
    async fn events_by_tx(
        &self,
        chain_id: i64,
        transaction_hash: &str,
    ) -> Result<Vec<IndexedEvent>, AppError>;
}
