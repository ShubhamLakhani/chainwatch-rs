//! Operational errors shared across layers.
//!
//! This enum does not depend on SQLx, reqwest, or Axum so domain and
//! application code can construct it. Infrastructure maps driver errors into
//! the `Database` / `Http` / `Rpc` variants.

use std::time::Duration;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("configuration error: {0}")]
    Config(String),

    #[error("RPC error: {0}")]
    Rpc(String),

    #[error("RPC request timed out after {0:?}")]
    RpcTimeout(Duration),

    #[error("database error: {0}")]
    Database(String),

    #[error("event decode error: {0}")]
    Decode(String),

    #[error("reorganization handling failed: {0}")]
    Reorg(String),

    #[error("invalid hex address '{0}'")]
    InvalidAddress(String),

    #[error("invalid hex value '{0}'")]
    InvalidHex(String),

    #[error("chain ID mismatch: configured {configured}, RPC reported {actual}")]
    ChainIdMismatch { configured: u64, actual: u64 },

    #[error("HTTP client error: {0}")]
    Http(String),

    #[error("I/O error: {0}")]
    Io(String),

    #[error("another indexer already holds the advisory lock for chain {0}")]
    IndexerLockHeld(u64),
}

impl AppError {
    pub fn config(msg: impl Into<String>) -> Self {
        Self::Config(msg.into())
    }

    pub fn rpc(msg: impl Into<String>) -> Self {
        Self::Rpc(msg.into())
    }

    pub fn database(error: impl ToString) -> Self {
        Self::Database(error.to_string())
    }

    pub fn http(error: impl ToString) -> Self {
        Self::Http(error.to_string())
    }

    pub fn io(error: impl ToString) -> Self {
        Self::Io(error.to_string())
    }
}

impl From<std::io::Error> for AppError {
    fn from(error: std::io::Error) -> Self {
        Self::io(error)
    }
}
