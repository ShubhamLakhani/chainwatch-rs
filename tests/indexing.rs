#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use chainwatch_rs::application::indexing::{next_block_to_index, safe_head};
use chainwatch_rs::application::ports::ChainReader;
use chainwatch_rs::infrastructure::blockchain::HttpRpcClient;
use chainwatch_rs::infrastructure::telemetry::Metrics;
use std::sync::Arc;
use std::time::Duration;

use common::unique_chain_id;

#[test]
fn safe_head_waits_for_confirmations() {
    assert_eq!(safe_head(100, 12), Some(88));
    assert_eq!(safe_head(12, 12), Some(0));
    assert_eq!(safe_head(11, 12), None);
}

#[test]
fn safe_head_zero_depth_follows_tip() {
    assert_eq!(safe_head(0, 0), Some(0));
    assert_eq!(safe_head(5, 0), Some(5));
}

#[test]
fn resume_from_checkpoint_not_start_block() {
    assert_eq!(next_block_to_index(Some(40), 0), 41);
    assert_eq!(next_block_to_index(Some(0), 100), 1);
}

#[test]
fn first_run_uses_start_block() {
    assert_eq!(next_block_to_index(None, 1_000), 1_000);
    assert_eq!(next_block_to_index(None, 0), 0);
}

#[test]
fn caught_up_when_next_exceeds_safe_head() {
    let checkpoint = Some(50);
    let latest = 52;
    let depth = 2;
    let next = next_block_to_index(checkpoint, 0);
    let safe = safe_head(latest, depth).unwrap();
    assert!(next > safe);
}

#[tokio::test]
#[ignore = "integration test: requires running Anvil node (RPC_URL)"]
async fn rpc_talks_to_anvil_when_available() {
    let url = common::required_rpc_url();
    let metrics = Arc::new(Metrics::new());
    let client = HttpRpcClient::new(url, 0, Duration::from_secs(2), metrics).expect("client");
    let chain_id = client
        .chain_id()
        .await
        .expect("RPC must be reachable and return chain_id");
    assert!(chain_id == 31337 || chain_id > 0);
    let genesis = client
        .get_block(0)
        .await
        .expect("get genesis block must succeed")
        .expect("genesis block must exist");
    assert_eq!(genesis.number, 0);
}

#[test]
fn always_compiles_without_infrastructure() {
    assert!(unique_chain_id() > 0);
}
