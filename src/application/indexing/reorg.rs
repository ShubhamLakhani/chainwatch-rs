//! Blockchain reorganization detection and rollback planning.

use crate::application::ports::{ChainReader, EventStore};
use crate::domain::{AppError, BlockHeader};
use std::collections::HashMap;
use tracing::{info, warn};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AncestorSearch {
    NoReorg,
    CommonAncestor {
        block_number: u64,
        block_hash: String,
        walked: u64,
    },
    NotFound {
        walked: u64,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RollbackPlan {
    pub ancestor_block: u64,
    pub ancestor_hash: String,
    pub diverged_at: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReorgAction {
    None,
    Rollback(RollbackPlan),
    /// Canonical genesis does not match stored genesis (for example a new Anvil process).
    ResetChain,
}

pub fn find_common_ancestor(
    from_block: u64,
    stored_tip_hash: &str,
    canonical_tip_hash: &str,
    max_depth: u64,
    stored: &HashMap<u64, String>,
    canonical: &HashMap<u64, String>,
) -> AncestorSearch {
    if stored_tip_hash.eq_ignore_ascii_case(canonical_tip_hash) {
        return AncestorSearch::NoReorg;
    }

    let mut walked = 0u64;
    let mut current = from_block;

    while walked < max_depth {
        if current == 0 {
            return match (stored.get(&0), canonical.get(&0)) {
                (Some(stored_hash), Some(canonical_hash))
                    if stored_hash.eq_ignore_ascii_case(canonical_hash) =>
                {
                    AncestorSearch::CommonAncestor {
                        block_number: 0,
                        block_hash: stored_hash.clone(),
                        walked: walked + 1,
                    }
                }
                _ => AncestorSearch::NotFound { walked: walked + 1 },
            };
        }

        current -= 1;
        walked += 1;

        match (stored.get(&current), canonical.get(&current)) {
            (Some(stored_hash), Some(canonical_hash))
                if stored_hash.eq_ignore_ascii_case(canonical_hash) =>
            {
                return AncestorSearch::CommonAncestor {
                    block_number: current,
                    block_hash: stored_hash.clone(),
                    walked,
                };
            }
            (None, _) => return AncestorSearch::NotFound { walked },
            _ => {}
        }
    }

    AncestorSearch::NotFound { walked }
}

pub async fn detect_reorg(
    rpc: &dyn ChainReader,
    repo: &dyn EventStore,
    chain_id: i64,
    max_depth: u64,
) -> Result<ReorgAction, AppError> {
    let Some(checkpoint) = repo.get_checkpoint(chain_id).await? else {
        return Ok(ReorgAction::None);
    };

    let from_block = checkpoint.last_indexed_block as u64;
    let on_chain = rpc.get_block(from_block).await?;
    let Some(canonical) = on_chain else {
        warn!(
            block = from_block,
            "checkpoint block is missing from RPC; treating as a reorg"
        );
        // Before walking back, check if genesis has changed (e.g. fresh Anvil process with same CHAIN_ID)
        if let (Some(stored_genesis), Some(canonical_genesis)) = (
            repo.get_indexed_block(chain_id, 0).await?,
            rpc.get_block(0).await?,
        ) {
            if !hashes_equal(&stored_genesis.block_hash, &canonical_genesis.hash_hex()) {
                warn!(
                    chain_id,
                    stored = %stored_genesis.block_hash,
                    canonical = %canonical_genesis.hash_hex(),
                    "stored genesis does not match the RPC genesis; resetting indexed data for this chain"
                );
                return Ok(ReorgAction::ResetChain);
            }
        }
        return walk_to_ancestor(rpc, repo, chain_id, from_block, max_depth).await;
    };

    if hashes_equal(&checkpoint.last_indexed_block_hash, &canonical.hash_hex()) {
        return Ok(ReorgAction::None);
    }

    // Check if genesis has changed before performing reorg walk
    if let (Some(stored_genesis), Some(canonical_genesis)) = (
        repo.get_indexed_block(chain_id, 0).await?,
        rpc.get_block(0).await?,
    ) {
        if !hashes_equal(&stored_genesis.block_hash, &canonical_genesis.hash_hex()) {
            warn!(
                chain_id,
                stored = %stored_genesis.block_hash,
                canonical = %canonical_genesis.hash_hex(),
                "stored genesis does not match the RPC genesis; resetting indexed data for this chain"
            );
            return Ok(ReorgAction::ResetChain);
        }
    }

    info!(
        block = from_block,
        stored = %checkpoint.last_indexed_block_hash,
        canonical = %canonical.hash_hex(),
        "checkpoint block hash no longer matches the canonical chain"
    );
    walk_to_ancestor(rpc, repo, chain_id, from_block, max_depth).await
}

async fn walk_to_ancestor(
    rpc: &dyn ChainReader,
    repo: &dyn EventStore,
    chain_id: i64,
    from_block: u64,
    max_depth: u64,
) -> Result<ReorgAction, AppError> {
    let mut walked = 0u64;
    let mut current = from_block;

    while walked < max_depth {
        if current == 0 {
            if let Some(stored) = repo.get_indexed_block(chain_id, 0).await? {
                if let Some(canonical) = rpc.get_block(0).await? {
                    if hashes_equal(&stored.block_hash, &canonical.hash_hex()) {
                        return Ok(ReorgAction::Rollback(RollbackPlan {
                            ancestor_block: 0,
                            ancestor_hash: stored.block_hash,
                            diverged_at: from_block,
                        }));
                    }
                }
            }
            warn!(
                chain_id,
                "stored genesis does not match the RPC genesis; resetting indexed data for this chain"
            );
            return Ok(ReorgAction::ResetChain);
        }

        current -= 1;
        walked += 1;

        let Some(stored) = repo.get_indexed_block(chain_id, current).await? else {
            return Err(AppError::Reorg(format!(
                "stored history ended at block {current} while searching for a common ancestor"
            )));
        };
        let Some(canonical) = rpc.get_block(current).await? else {
            continue;
        };
        if hashes_equal(&stored.block_hash, &canonical.hash_hex()) {
            return Ok(ReorgAction::Rollback(RollbackPlan {
                ancestor_block: current,
                ancestor_hash: stored.block_hash,
                diverged_at: from_block,
            }));
        }
    }

    // If no ancestor found within max_depth, check if genesis differs (chain replacement).
    if let (Some(stored_genesis), Some(canonical_genesis)) = (
        repo.get_indexed_block(chain_id, 0).await?,
        rpc.get_block(0).await?,
    ) {
        if !hashes_equal(&stored_genesis.block_hash, &canonical_genesis.hash_hex()) {
            warn!(
                chain_id,
                stored = %stored_genesis.block_hash,
                canonical = %canonical_genesis.hash_hex(),
                "no common ancestor within {max_depth} blocks and stored genesis differs from RPC; resetting chain"
            );
            return Ok(ReorgAction::ResetChain);
        }
    }

    Err(AppError::Reorg(format!(
        "no common ancestor within {max_depth} blocks of {from_block}"
    )))
}

pub fn parent_hash_mismatch(previous: Option<&BlockHeader>, next: &BlockHeader) -> bool {
    match previous {
        Some(prev) => prev.hash != next.parent_hash,
        None => false,
    }
}

fn hashes_equal(left: &str, right: &str) -> bool {
    left.eq_ignore_ascii_case(right)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map(pairs: &[(u64, &str)]) -> HashMap<u64, String> {
        pairs.iter().map(|(n, h)| (*n, (*h).to_string())).collect()
    }

    #[test]
    fn no_reorg_when_tip_matches() {
        let stored = map(&[(10, "0xaaa")]);
        let canonical = map(&[(10, "0xaaa")]);
        assert_eq!(
            find_common_ancestor(10, "0xaaa", "0xaaa", 16, &stored, &canonical),
            AncestorSearch::NoReorg
        );
    }

    #[test]
    fn finds_ancestor_below_diverged_tip() {
        let stored = map(&[(8, "0xaaa"), (9, "0xbbb"), (10, "0xold")]);
        let canonical = map(&[(8, "0xaaa"), (9, "0xbbb"), (10, "0xnew")]);
        match find_common_ancestor(10, "0xold", "0xnew", 16, &stored, &canonical) {
            AncestorSearch::CommonAncestor {
                block_number,
                block_hash,
                walked,
            } => {
                assert_eq!(block_number, 9);
                assert_eq!(block_hash, "0xbbb");
                assert_eq!(walked, 1);
            }
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn walks_to_genesis() {
        let stored = map(&[(0, "0xgen"), (1, "0xold1"), (2, "0xold2")]);
        let canonical = map(&[(0, "0xgen"), (1, "0xnew1"), (2, "0xnew2")]);
        match find_common_ancestor(2, "0xold2", "0xnew2", 16, &stored, &canonical) {
            AncestorSearch::CommonAncestor { block_number, .. } => {
                assert_eq!(block_number, 0);
            }
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn respects_safety_limit() {
        let stored = map(&[(1, "0xa"), (2, "0xb"), (3, "0xc")]);
        let canonical = map(&[(1, "0x1"), (2, "0x2"), (3, "0x3")]);
        match find_common_ancestor(3, "0xc", "0x3", 1, &stored, &canonical) {
            AncestorSearch::NotFound { walked } => assert_eq!(walked, 1),
            other => panic!("unexpected {other:?}"),
        }
    }
}
