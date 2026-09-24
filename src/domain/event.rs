//! ERC-20 transfer events and list filters.
//!
//! Token amounts stay as `U256`. Converting into `f64` would lose precision
//! for large values — the same class of bug as using JavaScript `Number`.

use alloy_primitives::{Address, B256, U256};
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawLog {
    pub address: Address,
    pub topics: Vec<B256>,
    pub data: Vec<u8>,
    pub block_number: u64,
    pub block_hash: B256,
    pub transaction_hash: B256,
    pub log_index: u32,
    pub removed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodedTransfer {
    pub chain_id: u64,
    pub contract: Address,
    pub sender: Address,
    pub recipient: Address,
    pub amount: U256,
    pub transaction_hash: B256,
    pub log_index: u32,
    pub block_number: u64,
    pub block_hash: B256,
    pub block_timestamp: DateTime<Utc>,
}

impl DecodedTransfer {
    pub fn identity_key(&self) -> EventIdentity {
        EventIdentity {
            chain_id: self.chain_id as i64,
            transaction_hash: format!("{:#x}", self.transaction_hash),
            log_index: self.log_index as i32,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct EventIdentity {
    pub chain_id: i64,
    pub transaction_hash: String,
    pub log_index: i32,
}

#[derive(Debug, Clone)]
pub struct EventListFilter {
    pub chain_id: i64,
    pub address: Option<String>,
    pub contract: Option<String>,
    pub limit: i64,
    pub offset: i64,
}

/// Query-side event record used by the HTTP API. Persistence maps rows into this type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexedEvent {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_key_matches_uniqueness_constraint() {
        let transfer = DecodedTransfer {
            chain_id: 31337,
            contract: Address::ZERO,
            sender: Address::ZERO,
            recipient: Address::ZERO,
            amount: U256::from(1u64),
            transaction_hash: B256::ZERO,
            log_index: 7,
            block_number: 1,
            block_hash: B256::ZERO,
            block_timestamp: DateTime::<Utc>::UNIX_EPOCH,
        };
        let key = transfer.identity_key();
        assert_eq!(key.chain_id, 31337);
        assert_eq!(key.log_index, 7);
        assert_eq!(
            key.transaction_hash,
            "0x0000000000000000000000000000000000000000000000000000000000000000"
        );
    }
}
