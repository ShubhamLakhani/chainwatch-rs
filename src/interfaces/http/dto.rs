use crate::domain::IndexedEvent;
use serde::{Deserialize, Serialize};

pub const DEFAULT_PAGE_LIMIT: u32 = 50;
pub const MAX_PAGE_LIMIT: u32 = 200;

#[derive(Debug, Deserialize)]
pub struct EventQueryParams {
    pub address: Option<String>,
    pub contract: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[derive(Serialize)]
pub struct HealthResponse {
    pub status: &'static str,
}

#[derive(Serialize)]
pub struct ReadyResponse {
    pub status: &'static str,
    pub database: bool,
    pub rpc: bool,
}

#[derive(Serialize)]
pub struct StatusResponse {
    pub chain_id: u64,
    pub confirmation_depth: u64,
    pub last_indexed_block: Option<i64>,
    pub last_indexed_block_hash: Option<String>,
    pub latest_observed_block: u64,
    pub safe_head: Option<u64>,
    pub indexing_lag: u64,
    pub catch_up_lag: u64,
    pub total_indexed_events: u64,
    pub rpc_request_failures: u64,
    pub retry_count: u64,
    pub reorg_count: u64,
    pub contract_address: Option<String>,
}

#[derive(Serialize)]
pub struct EventDto {
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
    pub block_timestamp: Option<chrono::DateTime<chrono::Utc>>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

impl From<IndexedEvent> for EventDto {
    fn from(event: IndexedEvent) -> Self {
        Self {
            id: event.id,
            chain_id: event.chain_id,
            contract_address: event.contract_address,
            transaction_hash: event.transaction_hash,
            log_index: event.log_index,
            sender_address: event.sender_address,
            recipient_address: event.recipient_address,
            amount: event.amount,
            block_number: event.block_number,
            block_hash: event.block_hash,
            block_timestamp: event.block_timestamp,
            created_at: event.created_at,
        }
    }
}

#[derive(Serialize)]
pub struct EventListResponse {
    pub events: Vec<EventDto>,
    pub limit: i64,
    pub offset: i64,
}
