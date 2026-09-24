use crate::domain::IndexedEvent;
use chrono::{DateTime, Utc};
use sqlx::FromRow;

#[derive(Debug, Clone, FromRow)]
pub struct IndexedEventRow {
    pub id: i64,
    pub chain_id: i64,
    pub contract_address: String,
    pub transaction_hash: String,
    pub log_index: i32,
    pub sender_address: String,
    pub recipient_address: String,
    pub amount: String,
    pub block_number: i64,
    pub block_hash: String,
    pub block_timestamp: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

impl From<IndexedEventRow> for IndexedEvent {
    fn from(row: IndexedEventRow) -> Self {
        Self {
            id: row.id,
            chain_id: row.chain_id,
            contract_address: row.contract_address,
            transaction_hash: row.transaction_hash,
            log_index: row.log_index,
            sender_address: row.sender_address,
            recipient_address: row.recipient_address,
            amount: row.amount,
            block_number: row.block_number,
            block_hash: row.block_hash,
            block_timestamp: row.block_timestamp,
            created_at: row.created_at,
        }
    }
}

#[derive(Debug, Clone, FromRow)]
pub struct IndexedBlockRow {
    pub chain_id: i64,
    pub block_number: i64,
    pub block_hash: String,
    pub parent_hash: String,
    pub indexed_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow)]
pub struct CheckpointRow {
    pub chain_id: i64,
    pub last_indexed_block: i64,
    pub last_indexed_block_hash: String,
    pub updated_at: DateTime<Utc>,
}
