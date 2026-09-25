//! ERC-20 `Transfer(address,address,uint256)` decoding.

use crate::domain::{AppError, DecodedTransfer, RawLog};
use alloy_primitives::{Address, B256, U256, b256, keccak256};
use chrono::{DateTime, Utc};

/// keccak256("Transfer(address,address,uint256)")
pub const ERC20_TRANSFER_TOPIC: B256 =
    b256!("ddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef");

pub fn transfer_event_signature_hash() -> B256 {
    keccak256(b"Transfer(address,address,uint256)")
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecodeOutcome {
    Transfer(DecodedTransfer),
    Skipped { reason: &'static str },
}

pub fn decode_erc20_transfer(
    log: &RawLog,
    chain_id: u64,
    block_timestamp: DateTime<Utc>,
) -> DecodeOutcome {
    if log.removed {
        return DecodeOutcome::Skipped {
            reason: "log marked removed by a reorg",
        };
    }
    if log.topics.len() != 3 {
        return DecodeOutcome::Skipped {
            reason: "not a 3-topic ERC-20 Transfer (ERC-721 uses 4 topics)",
        };
    }
    if log.topics[0] != ERC20_TRANSFER_TOPIC {
        return DecodeOutcome::Skipped {
            reason: "topic0 is not ERC-20 Transfer",
        };
    }
    if log.data.len() != 32 {
        return DecodeOutcome::Skipped {
            reason: "data is not a 32-byte uint256",
        };
    }

    let sender = address_from_topic(log.topics[1]);
    let recipient = address_from_topic(log.topics[2]);
    let amount = U256::from_be_slice(&log.data);

    DecodeOutcome::Transfer(DecodedTransfer {
        chain_id,
        contract: log.address,
        sender,
        recipient,
        amount,
        transaction_hash: log.transaction_hash,
        log_index: log.log_index,
        block_number: log.block_number,
        block_hash: log.block_hash,
        block_timestamp,
    })
}

pub fn decode_required(
    log: &RawLog,
    chain_id: u64,
    block_timestamp: DateTime<Utc>,
) -> Result<DecodedTransfer, AppError> {
    match decode_erc20_transfer(log, chain_id, block_timestamp) {
        DecodeOutcome::Transfer(decoded) => Ok(decoded),
        DecodeOutcome::Skipped { reason } => Err(AppError::Decode(reason.to_string())),
    }
}

fn address_from_topic(topic: B256) -> Address {
    Address::from_slice(&topic.as_slice()[12..32])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signature_hash_matches_canonical_topic() {
        assert_eq!(transfer_event_signature_hash(), ERC20_TRANSFER_TOPIC);
    }
}
