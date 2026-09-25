#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use alloy_primitives::{Address, B256};
use async_trait::async_trait;
use chainwatch_rs::application::indexing::{
    AncestorSearch, ReorgAction, RollbackPlan, detect_reorg, find_common_ancestor,
};
use chainwatch_rs::application::ports::{ChainReader, EventStore};
use chainwatch_rs::domain::{AppError, BlockHeader, RawLog};
use chainwatch_rs::infrastructure::persistence::Repository;
use sqlx::postgres::PgPoolOptions;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

use common::{cleanup_chain, test_header, test_transfer, unique_chain_id};

#[derive(Clone, Default)]
struct ChainState {
    blocks: HashMap<u64, BlockHeader>,
}

#[derive(Clone)]
struct MockRpc {
    state: Arc<Mutex<ChainState>>,
}

impl MockRpc {
    fn new(blocks: Vec<BlockHeader>) -> Self {
        let mut map = HashMap::new();
        for block in blocks {
            map.insert(block.number, block);
        }
        Self {
            state: Arc::new(Mutex::new(ChainState { blocks: map })),
        }
    }

    async fn replace_from(&self, new_blocks: Vec<BlockHeader>) {
        let mut state = self.state.lock().await;
        for block in new_blocks {
            state.blocks.insert(block.number, block);
        }
    }
}

#[async_trait]
impl ChainReader for MockRpc {
    async fn chain_id(&self) -> Result<u64, AppError> {
        Ok(31337)
    }

    async fn latest_block_number(&self) -> Result<u64, AppError> {
        let state = self.state.lock().await;
        Ok(state.blocks.keys().copied().max().unwrap_or(0))
    }

    async fn get_block(&self, number: u64) -> Result<Option<BlockHeader>, AppError> {
        let state = self.state.lock().await;
        Ok(state.blocks.get(&number).cloned())
    }

    async fn get_logs(
        &self,
        _from_block: u64,
        _to_block: u64,
        _contract: Option<Address>,
    ) -> Result<Vec<RawLog>, AppError> {
        Ok(Vec::new())
    }
}

fn block(number: u64, hash_byte: u8, parent_byte: u8) -> BlockHeader {
    BlockHeader {
        number,
        hash: B256::repeat_byte(hash_byte),
        parent_hash: B256::repeat_byte(parent_byte),
        timestamp: 1_700_000_000 + number,
    }
}

#[test]
fn ancestor_selection_picks_last_common_block() {
    let mut stored = HashMap::new();
    let mut canonical = HashMap::new();
    stored.insert(1, "0xaaa".into());
    stored.insert(2, "0xbbb".into());
    stored.insert(3, "0xold".into());
    canonical.insert(1, "0xaaa".into());
    canonical.insert(2, "0xbbb".into());
    canonical.insert(3, "0xnew".into());

    match find_common_ancestor(3, "0xold", "0xnew", 32, &stored, &canonical) {
        AncestorSearch::CommonAncestor {
            block_number,
            block_hash,
            walked,
        } => {
            assert_eq!(block_number, 2);
            assert_eq!(block_hash, "0xbbb");
            assert_eq!(walked, 1);
        }
        other => panic!("unexpected {other:?}"),
    }
}

#[test]
fn rollback_plan_from_search() {
    let plan = RollbackPlan {
        ancestor_block: 8,
        ancestor_hash: "0xabc".into(),
        diverged_at: 12,
    };
    assert!(plan.diverged_at > plan.ancestor_block);
}

#[tokio::test]
async fn mock_chain_can_replace_canonical_hashes() {
    let original = vec![
        block(1, 0x11, 0x00),
        block(2, 0x22, 0x11),
        block(3, 0x33, 0x22),
    ];
    let rpc = MockRpc::new(original);
    let before = rpc.get_block(3).await.unwrap().unwrap();
    assert_eq!(before.hash, B256::repeat_byte(0x33));

    rpc.replace_from(vec![block(3, 0x99, 0x22)]).await;
    let after = rpc.get_block(3).await.unwrap().unwrap();
    assert_eq!(after.hash, B256::repeat_byte(0x99));
    assert_ne!(before.hash, after.hash);
    assert_eq!(
        rpc.get_block(2).await.unwrap().unwrap().hash,
        B256::repeat_byte(0x22)
    );
}

#[test]
fn missing_history_is_not_found() {
    let stored = HashMap::from([(5u64, "0x5".to_string())]);
    let canonical = HashMap::from([(4u64, "0x4".to_string()), (5u64, "0xother".to_string())]);
    match find_common_ancestor(5, "0x5", "0xother", 16, &stored, &canonical) {
        AncestorSearch::NotFound { walked } => assert_eq!(walked, 1),
        other => panic!("unexpected {other:?}"),
    }
}

#[tokio::test]
#[ignore = "integration test: requires PostgreSQL (DATABASE_URL)"]
async fn reorg_rollback_uses_database_transaction() {
    let url = common::required_database_url();
    let pool = PgPoolOptions::new()
        .connect(&url)
        .await
        .expect("failed to connect to PostgreSQL");
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("migrate");
    let repo = Repository::new(pool.clone());
    let chain_id = unique_chain_id();

    let b1 = test_header(1, 0x11);
    let b2 = test_header(2, 0x22);
    let b3 = test_header(3, 0x33);
    repo.persist_block(chain_id, &b1, &[test_transfer(&b1, 0, 1)])
        .await
        .unwrap();
    repo.persist_block(chain_id, &b2, &[test_transfer(&b2, 0, 2)])
        .await
        .unwrap();
    repo.persist_block(chain_id, &b3, &[test_transfer(&b3, 0, 3)])
        .await
        .unwrap();

    let mut live = vec![b1.clone(), b2.clone(), b3.clone()];
    live[2] = test_header(3, 0x99);
    let rpc = MockRpc::new(live);
    rpc.replace_from(vec![test_header(3, 0x99)]).await;

    let action = detect_reorg(&rpc, &repo, chain_id, 16)
        .await
        .expect("detect");
    let ReorgAction::Rollback(plan) = action else {
        panic!("expected rollback, got {action:?}");
    };
    assert_eq!(plan.ancestor_block, 2);

    let stats = repo
        .rollback_after(chain_id, plan.ancestor_block, &plan.ancestor_hash)
        .await
        .expect("rollback");
    assert_eq!(stats.deleted_events, 1);
    assert_eq!(stats.deleted_blocks, 1);

    assert_eq!(repo.count_events(chain_id).await.unwrap(), 2);
    let checkpoint = repo.get_checkpoint(chain_id).await.unwrap().unwrap();
    assert_eq!(checkpoint.last_indexed_block, 2);

    cleanup_chain(&pool, chain_id).await;
}
