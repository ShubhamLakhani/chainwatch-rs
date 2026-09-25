//! JSON-RPC HTTP client for an EVM node.

use crate::application::ports::ChainReader;
use crate::domain::{AppError, BlockHeader, RawLog};
use crate::infrastructure::blockchain::decoder::ERC20_TRANSFER_TOPIC;
use crate::infrastructure::blockchain::retry::retry_with_backoff;
use crate::infrastructure::telemetry::metrics::Metrics;
use alloy_primitives::{Address, B256};
use async_trait::async_trait;
use serde_json::{Value, json};
use std::sync::Arc;
use std::time::Duration;
use tracing::debug;

#[derive(Clone)]
pub struct HttpRpcClient {
    http: reqwest::Client,
    url: String,
    max_retries: u32,
    timeout: Duration,
    metrics: Arc<Metrics>,
}

impl HttpRpcClient {
    pub fn new(
        url: String,
        max_retries: u32,
        timeout: Duration,
        metrics: Arc<Metrics>,
    ) -> Result<Self, AppError> {
        let http = reqwest::Client::builder()
            .timeout(timeout)
            .build()
            .map_err(AppError::http)?;
        Ok(Self {
            http,
            url,
            max_retries,
            timeout,
            metrics,
        })
    }

    async fn call(&self, method: &str, params: Value) -> Result<Value, AppError> {
        let method_owned = method.to_string();
        let params = params.clone();
        let http = self.http.clone();
        let url = self.url.clone();
        let metrics = Arc::clone(&self.metrics);
        let timeout = self.timeout;

        retry_with_backoff(self.max_retries, timeout, &metrics, || {
            let http = http.clone();
            let url = url.clone();
            let method_owned = method_owned.clone();
            let params = params.clone();
            async move { rpc_once(&http, &url, &method_owned, params).await }
        })
        .await
    }
}

#[async_trait]
impl ChainReader for HttpRpcClient {
    async fn chain_id(&self) -> Result<u64, AppError> {
        let result = self.call("eth_chainId", json!([])).await?;
        parse_hex_u64_value(&result)
    }

    async fn latest_block_number(&self) -> Result<u64, AppError> {
        let result = self.call("eth_blockNumber", json!([])).await?;
        parse_hex_u64_value(&result)
    }

    async fn get_block(&self, number: u64) -> Result<Option<BlockHeader>, AppError> {
        let result = self
            .call("eth_getBlockByNumber", json!([to_hex_u64(number), false]))
            .await?;
        if result.is_null() {
            return Ok(None);
        }
        Ok(Some(parse_block_header(&result)?))
    }

    async fn get_logs(
        &self,
        from_block: u64,
        to_block: u64,
        contract: Option<Address>,
    ) -> Result<Vec<RawLog>, AppError> {
        let mut filter = json!({
            "fromBlock": to_hex_u64(from_block),
            "toBlock": to_hex_u64(to_block),
            "topics": [format!("{ERC20_TRANSFER_TOPIC:#x}")],
        });
        if let Some(address) = contract {
            filter["address"] = json!(format!("{address:#x}"));
        }
        let result = self.call("eth_getLogs", json!([filter])).await?;
        let logs = result
            .as_array()
            .ok_or_else(|| AppError::rpc("eth_getLogs did not return an array"))?;
        logs.iter().map(parse_raw_log).collect()
    }
}

async fn rpc_once(
    http: &reqwest::Client,
    url: &str,
    method: &str,
    params: Value,
) -> Result<Value, AppError> {
    let body = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": method,
        "params": params,
    });
    debug!(method, "sending JSON-RPC request");
    let response = http
        .post(url)
        .json(&body)
        .send()
        .await
        .map_err(AppError::http)?;
    let status = response.status();
    let payload: Value = response
        .json()
        .await
        .map_err(|e| AppError::rpc(format!("invalid JSON-RPC response ({status}): {e}")))?;
    if let Some(error) = payload.get("error") {
        return Err(AppError::rpc(format!("{method} failed: {error}")));
    }
    payload
        .get("result")
        .cloned()
        .ok_or_else(|| AppError::rpc(format!("{method} response missing result")))
}

pub fn to_hex_u64(value: u64) -> String {
    format!("{value:#x}")
}

pub fn parse_hex_u64(raw: &str) -> Result<u64, AppError> {
    let trimmed = raw.trim().trim_start_matches("0x").trim_start_matches("0X");
    if trimmed.is_empty() {
        return Ok(0);
    }
    u64::from_str_radix(trimmed, 16).map_err(|_| AppError::InvalidHex(raw.to_string()))
}

fn parse_hex_u64_value(value: &Value) -> Result<u64, AppError> {
    match value {
        Value::String(s) => parse_hex_u64(s),
        Value::Number(n) => n
            .as_u64()
            .ok_or_else(|| AppError::rpc(format!("integer out of range: {n}"))),
        other => Err(AppError::rpc(format!("expected hex quantity, got {other}"))),
    }
}

fn parse_hex_bytes(raw: &str) -> Result<Vec<u8>, AppError> {
    let trimmed = raw.trim().trim_start_matches("0x").trim_start_matches("0X");
    hex::decode(trimmed).map_err(|_| AppError::InvalidHex(raw.to_string()))
}

fn parse_b256(raw: &str) -> Result<B256, AppError> {
    let bytes = parse_hex_bytes(raw)?;
    if bytes.len() != 32 {
        return Err(AppError::InvalidHex(raw.to_string()));
    }
    Ok(B256::from_slice(&bytes))
}

fn parse_address(raw: &str) -> Result<Address, AppError> {
    let bytes = parse_hex_bytes(raw)?;
    if bytes.len() != 20 {
        return Err(AppError::InvalidAddress(raw.to_string()));
    }
    Ok(Address::from_slice(&bytes))
}

fn json_string<'a>(value: &'a Value, field: &str) -> Result<&'a str, AppError> {
    value
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| AppError::rpc(format!("missing string field '{field}'")))
}

fn parse_block_header(value: &Value) -> Result<BlockHeader, AppError> {
    Ok(BlockHeader {
        number: parse_hex_u64(json_string(value, "number")?)?,
        hash: parse_b256(json_string(value, "hash")?)?,
        parent_hash: parse_b256(json_string(value, "parentHash")?)?,
        timestamp: parse_hex_u64(json_string(value, "timestamp")?)?,
    })
}

fn parse_raw_log(value: &Value) -> Result<RawLog, AppError> {
    let topics = value
        .get("topics")
        .and_then(Value::as_array)
        .ok_or_else(|| AppError::rpc("log missing topics"))?
        .iter()
        .map(|topic| {
            topic
                .as_str()
                .ok_or_else(|| AppError::rpc("topic is not a string"))
                .and_then(parse_b256)
        })
        .collect::<Result<Vec<_>, _>>()?;

    let log_index = match value.get("logIndex") {
        Some(Value::String(s)) => parse_hex_u64(s)? as u32,
        Some(Value::Number(n)) => {
            n.as_u64()
                .ok_or_else(|| AppError::rpc("logIndex out of range"))? as u32
        }
        _ => return Err(AppError::rpc("log missing logIndex")),
    };

    Ok(RawLog {
        address: parse_address(json_string(value, "address")?)?,
        topics,
        data: parse_hex_bytes(json_string(value, "data")?)?,
        block_number: parse_hex_u64(json_string(value, "blockNumber")?)?,
        block_hash: parse_b256(json_string(value, "blockHash")?)?,
        transaction_hash: parse_b256(json_string(value, "transactionHash")?)?,
        log_index,
        removed: value
            .get("removed")
            .and_then(Value::as_bool)
            .unwrap_or(false),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_hex_quantities() {
        assert_eq!(parse_hex_u64("0xa").unwrap(), 10);
        assert_eq!(parse_hex_u64("0x0").unwrap(), 0);
        assert_eq!(to_hex_u64(255), "0xff");
    }
}
