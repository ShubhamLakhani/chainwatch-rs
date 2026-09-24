use chrono::{DateTime, Utc};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Checkpoint {
    pub chain_id: i64,
    pub last_indexed_block: i64,
    pub last_indexed_block_hash: String,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct PersistOutcome {
    pub inserted_events: u64,
    pub duplicate_events: u64,
}
