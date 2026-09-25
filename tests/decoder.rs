#![allow(clippy::unwrap_used, clippy::expect_used)]

use alloy_primitives::{Address, B256, U256};
use chainwatch_rs::domain::RawLog;
use chainwatch_rs::infrastructure::blockchain::{
    DecodeOutcome, ERC20_TRANSFER_TOPIC, decode_erc20_transfer, decode_required,
    transfer_event_signature_hash,
};
use chrono::{DateTime, Utc};

fn padded_address(byte: u8) -> B256 {
    let mut buf = [0u8; 32];
    buf[31] = byte;
    B256::from(buf)
}

fn transfer_log(from: u8, to: u8, amount: u64, log_index: u32) -> RawLog {
    let mut data = [0u8; 32];
    data[24..].copy_from_slice(&amount.to_be_bytes());
    RawLog {
        address: Address::repeat_byte(0x11),
        topics: vec![
            ERC20_TRANSFER_TOPIC,
            padded_address(from),
            padded_address(to),
        ],
        data: data.to_vec(),
        block_number: 15,
        block_hash: B256::repeat_byte(0x22),
        transaction_hash: B256::repeat_byte(0x33),
        log_index,
        removed: false,
    }
}

#[test]
fn canonical_keccak_matches_known_topic() {
    assert_eq!(
        format!("{ERC20_TRANSFER_TOPIC:#x}"),
        "0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef"
    );
    assert_eq!(transfer_event_signature_hash(), ERC20_TRANSFER_TOPIC);
}

#[test]
fn decodes_standard_erc20_transfer() {
    let log = transfer_log(0xaa, 0xbb, 1_000_000_000_000_000_000, 3);
    let decoded = decode_required(&log, 31337, DateTime::<Utc>::UNIX_EPOCH).expect("decode");
    assert_eq!(decoded.chain_id, 31337);
    assert_eq!(
        format!("{:#x}", decoded.sender),
        "0x00000000000000000000000000000000000000aa"
    );
    assert_eq!(
        format!("{:#x}", decoded.recipient),
        "0x00000000000000000000000000000000000000bb"
    );
    assert_eq!(decoded.amount, U256::from(1_000_000_000_000_000_000u64));
    assert_eq!(decoded.log_index, 3);
    assert_eq!(decoded.amount.to_string(), "1000000000000000000");
}

#[test]
fn skips_erc721_four_topic_transfer() {
    let mut log = transfer_log(1, 2, 7, 0);
    log.topics.push(B256::repeat_byte(0x01));
    match decode_erc20_transfer(&log, 1, DateTime::<Utc>::UNIX_EPOCH) {
        DecodeOutcome::Skipped { reason } => {
            assert!(reason.contains("4 topics"));
        }
        other => panic!("expected skip, got {other:?}"),
    }
}

#[test]
fn skips_removed_logs() {
    let mut log = transfer_log(1, 2, 1, 0);
    log.removed = true;
    assert!(matches!(
        decode_erc20_transfer(&log, 1, DateTime::<Utc>::UNIX_EPOCH),
        DecodeOutcome::Skipped { .. }
    ));
}

#[test]
fn skips_wrong_topic0() {
    let mut log = transfer_log(1, 2, 1, 0);
    log.topics[0] = B256::repeat_byte(0xff);
    assert!(matches!(
        decode_erc20_transfer(&log, 1, DateTime::<Utc>::UNIX_EPOCH),
        DecodeOutcome::Skipped { .. }
    ));
}

#[test]
fn skips_non_uint256_data() {
    let mut log = transfer_log(1, 2, 1, 0);
    log.data = vec![0x00];
    assert!(matches!(
        decode_erc20_transfer(&log, 1, DateTime::<Utc>::UNIX_EPOCH),
        DecodeOutcome::Skipped { .. }
    ));
}

#[test]
fn large_uint256_is_not_converted_to_float() {
    let mut log = transfer_log(1, 2, 0, 0);
    log.data = [0xffu8; 32].to_vec();
    let decoded = decode_required(&log, 1, DateTime::<Utc>::UNIX_EPOCH).expect("decode");
    assert_eq!(
        decoded.amount.to_string(),
        "115792089237316195423570985008687907853269984665640564039457584007913129639935"
    );
}
