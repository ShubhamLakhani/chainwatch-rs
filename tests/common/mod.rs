#![allow(dead_code, clippy::unwrap_used, clippy::expect_used)]

use alloy_primitives::{Address, B256, U256};
use chainwatch_rs::domain::{BlockHeader, DecodedTransfer};

pub fn database_url() -> Option<String> {
    dotenvy::dotenv().ok();
    std::env::var("DATABASE_URL")
        .ok()
        .filter(|url| !url.is_empty())
}

pub fn required_database_url() -> String {
    database_url().expect("DATABASE_URL environment variable must be set for integration tests")
}

pub fn rpc_url() -> Option<String> {
    dotenvy::dotenv().ok();
    std::env::var("RPC_URL").ok().filter(|url| !url.is_empty())
}

pub fn required_rpc_url() -> String {
    rpc_url().expect("RPC_URL environment variable must be set for integration tests")
}

pub fn unique_chain_id() -> i64 {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    50_000 + (nanos % 1_000_000) as i64
}

pub fn test_header(number: u64, hash_byte: u8) -> BlockHeader {
    BlockHeader {
        number,
        hash: B256::repeat_byte(hash_byte),
        parent_hash: B256::repeat_byte(hash_byte.saturating_sub(1)),
        timestamp: 1_700_000_000 + number,
    }
}

pub fn test_transfer(header: &BlockHeader, log_index: u32, amount: u64) -> DecodedTransfer {
    let mut tx = [0u8; 32];
    tx[0] = 0xab;
    tx[16..24].copy_from_slice(&header.number.to_be_bytes());
    tx[24..28].copy_from_slice(&log_index.to_be_bytes());
    DecodedTransfer {
        chain_id: 31337,
        contract: Address::repeat_byte(0x42),
        sender: Address::repeat_byte(0x01),
        recipient: Address::repeat_byte(0x02),
        amount: U256::from(amount),
        transaction_hash: B256::from(tx),
        log_index,
        block_number: header.number,
        block_hash: header.hash,
        block_timestamp: header.timestamp_utc(),
    }
}

pub async fn cleanup_chain(pool: &sqlx::PgPool, chain_id: i64) {
    sqlx::query("DELETE FROM indexed_events WHERE chain_id = $1")
        .bind(chain_id)
        .execute(pool)
        .await
        .ok();
    sqlx::query("DELETE FROM indexed_blocks WHERE chain_id = $1")
        .bind(chain_id)
        .execute(pool)
        .await
        .ok();
    sqlx::query("DELETE FROM indexer_checkpoints WHERE chain_id = $1")
        .bind(chain_id)
        .execute(pool)
        .await
        .ok();
}
