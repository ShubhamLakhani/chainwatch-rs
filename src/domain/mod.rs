//! Blockchain types and invariants with no Axum, SQLx, or HTTP dependencies.
//!
//! Node.js analogue: a `types.ts` module that never imports Express or `pg`.

pub mod block;
pub mod chain;
pub mod checkpoint;
pub mod error;
pub mod event;
pub mod hexutil;

pub use block::BlockHeader;
pub use chain::{ChainId, next_block_to_index, safe_head};
pub use checkpoint::{Checkpoint, PersistOutcome};
pub use error::AppError;
pub use event::{DecodedTransfer, EventIdentity, EventListFilter, IndexedEvent, RawLog};
pub use hexutil::{is_address, is_tx_hash, normalize_hex};
