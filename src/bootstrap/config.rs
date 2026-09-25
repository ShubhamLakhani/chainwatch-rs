//! Environment-based configuration with fail-fast validation.

use crate::domain::AppError;
use alloy_primitives::Address;
use std::str::FromStr;
use std::time::Duration;
use url::Url;

#[derive(Debug, Clone)]
pub struct Config {
    pub database_url: String,
    pub rpc_url: String,
    pub chain_id: u64,
    pub start_block: u64,
    pub confirmation_depth: u64,
    pub poll_interval: Duration,
    pub max_concurrent_requests: usize,
    pub max_retries: u32,
    pub request_timeout: Duration,
    pub batch_size: u64,
    pub contract_address: Option<Address>,
    pub api_host: String,
    pub api_port: u16,
    pub max_reorg_depth: u64,
}

impl Config {
    pub fn from_env() -> Result<Self, AppError> {
        let database_url = required("DATABASE_URL")?;
        validate_database_url(&database_url)?;

        let rpc_url = required("RPC_URL")?;
        validate_rpc_url(&rpc_url)?;

        let chain_id = parse_u64("CHAIN_ID", "31337")?;
        let start_block = parse_u64("START_BLOCK", "0")?;
        let confirmation_depth = parse_u64("CONFIRMATION_DEPTH", "2")?;
        let poll_interval_ms = parse_u64("POLL_INTERVAL_MS", "1000")?;
        let max_concurrent_requests = parse_usize("MAX_CONCURRENT_REQUESTS", "8")?;
        let max_retries = parse_u32("MAX_RETRIES", "5")?;
        let request_timeout_seconds = parse_u64("REQUEST_TIMEOUT_SECONDS", "10")?;
        let batch_size = parse_u64("BATCH_SIZE", "20")?;
        let max_reorg_depth = parse_u64("MAX_REORG_DEPTH", "128")?;
        let api_host = optional("API_HOST", "127.0.0.1");
        let api_port = parse_u16("API_PORT", "8080")?;
        let contract_address = parse_optional_address("CONTRACT_ADDRESS")?;

        if poll_interval_ms == 0 {
            return Err(AppError::config("POLL_INTERVAL_MS must be greater than 0"));
        }
        if max_concurrent_requests == 0 {
            return Err(AppError::config(
                "MAX_CONCURRENT_REQUESTS must be greater than 0",
            ));
        }
        if batch_size == 0 {
            return Err(AppError::config("BATCH_SIZE must be greater than 0"));
        }
        if request_timeout_seconds == 0 {
            return Err(AppError::config(
                "REQUEST_TIMEOUT_SECONDS must be greater than 0",
            ));
        }
        if max_reorg_depth == 0 {
            return Err(AppError::config("MAX_REORG_DEPTH must be greater than 0"));
        }

        Ok(Self {
            database_url,
            rpc_url,
            chain_id,
            start_block,
            confirmation_depth,
            poll_interval: Duration::from_millis(poll_interval_ms),
            max_concurrent_requests,
            max_retries,
            request_timeout: Duration::from_secs(request_timeout_seconds),
            batch_size,
            contract_address,
            api_host,
            api_port,
            max_reorg_depth,
        })
    }

    pub fn api_bind_addr(&self) -> String {
        format!("{}:{}", self.api_host, self.api_port)
    }

    pub fn log_summary(&self) -> String {
        format!(
            "rpc_url={} chain_id={} start_block={} confirmation_depth={} poll_interval_ms={} max_concurrent_requests={} max_retries={} request_timeout_secs={} batch_size={} contract_address={} api={} database_url={} max_reorg_depth={}",
            self.rpc_url,
            self.chain_id,
            self.start_block,
            self.confirmation_depth,
            self.poll_interval.as_millis(),
            self.max_concurrent_requests,
            self.max_retries,
            self.request_timeout.as_secs(),
            self.batch_size,
            self.contract_address
                .map(|addr| format!("{addr:#x}"))
                .unwrap_or_else(|| "(all contracts)".to_string()),
            self.api_bind_addr(),
            redact_database_url(&self.database_url),
            self.max_reorg_depth,
        )
    }
}

fn required(key: &str) -> Result<String, AppError> {
    std::env::var(key).map_err(|_| AppError::config(format!("{key} is required")))
}

fn optional(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_string())
}

fn parse_u64(key: &str, default: &str) -> Result<u64, AppError> {
    let raw = optional(key, default);
    raw.parse::<u64>()
        .map_err(|_| AppError::config(format!("{key} must be an integer, got '{raw}'")))
}

fn parse_u32(key: &str, default: &str) -> Result<u32, AppError> {
    let raw = optional(key, default);
    raw.parse::<u32>()
        .map_err(|_| AppError::config(format!("{key} must be an integer, got '{raw}'")))
}

fn parse_u16(key: &str, default: &str) -> Result<u16, AppError> {
    let raw = optional(key, default);
    raw.parse::<u16>()
        .map_err(|_| AppError::config(format!("{key} must be a port number, got '{raw}'")))
}

fn parse_usize(key: &str, default: &str) -> Result<usize, AppError> {
    let raw = optional(key, default);
    raw.parse::<usize>()
        .map_err(|_| AppError::config(format!("{key} must be an integer, got '{raw}'")))
}

fn parse_optional_address(key: &str) -> Result<Option<Address>, AppError> {
    let raw = std::env::var(key).unwrap_or_default();
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    Address::from_str(trimmed)
        .map(Some)
        .map_err(|_| AppError::InvalidAddress(trimmed.to_string()))
}

fn validate_database_url(raw: &str) -> Result<(), AppError> {
    let parsed = Url::parse(raw)
        .map_err(|e| AppError::config(format!("DATABASE_URL is not a valid URL: {e}")))?;
    match parsed.scheme() {
        "postgres" | "postgresql" => Ok(()),
        other => Err(AppError::config(format!(
            "DATABASE_URL must use postgres://, got '{other}'"
        ))),
    }
}

fn validate_rpc_url(raw: &str) -> Result<(), AppError> {
    let parsed = Url::parse(raw)
        .map_err(|e| AppError::config(format!("RPC_URL is not a valid URL: {e}")))?;
    match parsed.scheme() {
        "http" | "https" => Ok(()),
        other => Err(AppError::config(format!(
            "RPC_URL must use http:// or https://, got '{other}'"
        ))),
    }
}

pub fn redact_database_url(raw: &str) -> String {
    match Url::parse(raw) {
        Ok(mut url) => {
            if url.password().is_some() {
                let _ = url.set_password(Some("***"));
            }
            url.to_string()
        }
        Err(_) => "<invalid-database-url>".to_string(),
    }
}

#[cfg(test)]
#[allow(unsafe_code, clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn with_env<F: FnOnce()>(pairs: &[(&str, Option<&str>)], f: F) {
        let _guard = ENV_LOCK.lock().expect("env lock");
        let keys = [
            "DATABASE_URL",
            "RPC_URL",
            "CHAIN_ID",
            "START_BLOCK",
            "CONFIRMATION_DEPTH",
            "POLL_INTERVAL_MS",
            "MAX_CONCURRENT_REQUESTS",
            "MAX_RETRIES",
            "REQUEST_TIMEOUT_SECONDS",
            "BATCH_SIZE",
            "CONTRACT_ADDRESS",
            "API_HOST",
            "API_PORT",
            "MAX_REORG_DEPTH",
        ];
        let previous: Vec<(String, Option<String>)> = keys
            .iter()
            .map(|k| ((*k).to_string(), std::env::var(k).ok()))
            .collect();
        for key in keys {
            // SAFETY: tests serialize environment mutation with ENV_LOCK.
            unsafe { std::env::remove_var(key) };
        }
        for (key, value) in pairs {
            match value {
                Some(v) => unsafe { std::env::set_var(key, v) },
                None => unsafe { std::env::remove_var(key) },
            }
        }
        f();
        for (key, value) in previous {
            match value {
                Some(v) => unsafe { std::env::set_var(key, v) },
                None => unsafe { std::env::remove_var(key) },
            }
        }
    }

    #[test]
    fn loads_defaults_and_required_values() {
        with_env(
            &[
                (
                    "DATABASE_URL",
                    Some("postgres://chainwatch:secret@localhost:5432/chainwatch"),
                ),
                ("RPC_URL", Some("http://127.0.0.1:8545")),
            ],
            || {
                let cfg = Config::from_env().expect("config");
                assert_eq!(cfg.chain_id, 31337);
                assert_eq!(cfg.confirmation_depth, 2);
                assert_eq!(cfg.batch_size, 20);
                assert!(cfg.contract_address.is_none());
                assert!(cfg.log_summary().contains("***"));
                assert!(!cfg.log_summary().contains("secret"));
            },
        );
    }

    #[test]
    fn rejects_zero_concurrency() {
        with_env(
            &[
                ("DATABASE_URL", Some("postgres://localhost/chainwatch")),
                ("RPC_URL", Some("http://127.0.0.1:8545")),
                ("MAX_CONCURRENT_REQUESTS", Some("0")),
            ],
            || {
                let err = Config::from_env().expect_err("should fail");
                assert!(err.to_string().contains("MAX_CONCURRENT_REQUESTS"));
            },
        );
    }

    #[test]
    fn parses_contract_address() {
        with_env(
            &[
                ("DATABASE_URL", Some("postgres://localhost/chainwatch")),
                ("RPC_URL", Some("http://127.0.0.1:8545")),
                (
                    "CONTRACT_ADDRESS",
                    Some("0x0000000000000000000000000000000000000001"),
                ),
            ],
            || {
                let cfg = Config::from_env().expect("config");
                assert_eq!(
                    format!("{:#x}", cfg.contract_address.expect("addr")),
                    "0x0000000000000000000000000000000000000001"
                );
            },
        );
    }

    #[test]
    fn rejects_invalid_contract_address() {
        with_env(
            &[
                ("DATABASE_URL", Some("postgres://localhost/chainwatch")),
                ("RPC_URL", Some("http://127.0.0.1:8545")),
                ("CONTRACT_ADDRESS", Some("not-an-address")),
            ],
            || {
                let err = Config::from_env().expect_err("should fail");
                assert!(matches!(err, AppError::InvalidAddress(_)));
            },
        );
    }
}
