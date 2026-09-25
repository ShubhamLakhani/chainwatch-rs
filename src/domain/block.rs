use alloy_primitives::B256;
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockHeader {
    pub number: u64,
    pub hash: B256,
    pub parent_hash: B256,
    pub timestamp: u64,
}

impl BlockHeader {
    pub fn hash_hex(&self) -> String {
        format!("{:#x}", self.hash)
    }

    pub fn parent_hash_hex(&self) -> String {
        format!("{:#x}", self.parent_hash)
    }

    pub fn timestamp_utc(&self) -> DateTime<Utc> {
        DateTime::<Utc>::from_timestamp(self.timestamp as i64, 0)
            .unwrap_or(DateTime::<Utc>::UNIX_EPOCH)
    }
}
