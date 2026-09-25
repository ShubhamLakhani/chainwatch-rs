//! Single-crate EVM ERC-20 Transfer indexer.
//!
//! Layers: `domain` → `application` → `infrastructure` / `interfaces`, wired by `bootstrap`.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

pub mod application;
pub mod bootstrap;
pub mod domain;
pub mod infrastructure;
pub mod interfaces;

pub use bootstrap::run;
pub use domain::AppError;
