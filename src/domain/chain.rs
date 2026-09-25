//! Chain identity and checkpoint arithmetic.

/// Newtype for an EVM chain id. Stored as `u64` to match `eth_chainId`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ChainId(pub u64);

impl ChainId {
    pub fn as_i64(self) -> i64 {
        self.0 as i64
    }
}

impl From<u64> for ChainId {
    fn from(value: u64) -> Self {
        Self(value)
    }
}

/// Highest block that is considered final enough to index.
pub fn safe_head(latest_block: u64, confirmation_depth: u64) -> Option<u64> {
    latest_block.checked_sub(confirmation_depth)
}

/// Next block to request. A stored checkpoint always wins over `START_BLOCK`.
pub fn next_block_to_index(checkpoint_block: Option<u64>, start_block: u64) -> u64 {
    match checkpoint_block {
        Some(block) => block.saturating_add(1),
        None => start_block,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn confirmation_depth_zero_indexes_tip() {
        assert_eq!(safe_head(12, 0), Some(12));
        assert_eq!(safe_head(12, 2), Some(10));
        assert_eq!(safe_head(1, 2), None);
        assert_eq!(safe_head(0, 0), Some(0));
    }

    #[test]
    fn resume_prefers_checkpoint() {
        assert_eq!(next_block_to_index(None, 5), 5);
        assert_eq!(next_block_to_index(Some(9), 0), 10);
        assert_eq!(next_block_to_index(Some(0), 99), 1);
    }
}
