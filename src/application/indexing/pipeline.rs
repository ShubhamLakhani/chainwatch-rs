//! Bounded concurrent fetch pipeline.
//!
//! Independent block headers are fetched in parallel, but a Tokio `Semaphore`
//! caps how many RPC calls run at once. Decoded events are grouped by block
//! and returned sorted so the indexer can persist them in chain order.

use crate::application::ports::ChainReader;
use crate::bootstrap::config::Config;
use crate::domain::{AppError, BlockHeader, DecodedTransfer, RawLog};
use crate::infrastructure::blockchain::decoder::{DecodeOutcome, decode_erc20_transfer};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Semaphore;
use tokio::task::JoinSet;
use tracing::{debug, warn};

#[derive(Debug, Clone)]
pub struct BlockBatch {
    pub header: BlockHeader,
    pub events: Vec<DecodedTransfer>,
}

pub struct Processor {
    rpc: Arc<dyn ChainReader>,
    config: Arc<Config>,
    semaphore: Arc<Semaphore>,
}

impl Processor {
    pub fn new(rpc: Arc<dyn ChainReader>, config: Arc<Config>) -> Self {
        let semaphore = Arc::new(Semaphore::new(config.max_concurrent_requests));
        Self {
            rpc,
            config,
            semaphore,
        }
    }

    pub async fn fetch_range(
        &self,
        from_block: u64,
        to_block: u64,
    ) -> Result<Vec<BlockBatch>, AppError> {
        if from_block > to_block {
            return Ok(Vec::new());
        }

        let logs_rpc = Arc::clone(&self.rpc);
        let logs_sem = Arc::clone(&self.semaphore);
        let contract = self.config.contract_address;
        let logs_fut = async move {
            let _permit = logs_sem
                .acquire_owned()
                .await
                .map_err(|_| AppError::rpc("RPC semaphore closed"))?;
            logs_rpc.get_logs(from_block, to_block, contract).await
        };

        let headers_fut = self.fetch_headers(from_block, to_block);
        let (logs, headers) = tokio::try_join!(logs_fut, headers_fut)?;

        let mut logs_by_block: HashMap<u64, Vec<RawLog>> = HashMap::new();
        for log in logs {
            logs_by_block.entry(log.block_number).or_default().push(log);
        }

        let mut batches = Vec::with_capacity(headers.len());
        for header in headers {
            let mut events = Vec::new();
            if let Some(block_logs) = logs_by_block.remove(&header.number) {
                for log in block_logs {
                    match decode_erc20_transfer(&log, self.config.chain_id, header.timestamp_utc())
                    {
                        DecodeOutcome::Transfer(decoded) => events.push(decoded),
                        DecodeOutcome::Skipped { reason } => {
                            debug!(
                                block = header.number,
                                tx = %format!("{:#x}", log.transaction_hash),
                                reason,
                                "skipped log"
                            );
                        }
                    }
                }
            }
            events.sort_by_key(|event| event.log_index);
            batches.push(BlockBatch { header, events });
        }

        Ok(batches)
    }

    async fn fetch_headers(
        &self,
        from_block: u64,
        to_block: u64,
    ) -> Result<Vec<BlockHeader>, AppError> {
        let mut join_set: JoinSet<Result<(u64, Option<BlockHeader>), AppError>> = JoinSet::new();

        for number in from_block..=to_block {
            let rpc = Arc::clone(&self.rpc);
            let semaphore = Arc::clone(&self.semaphore);
            join_set.spawn(async move {
                let _permit = semaphore
                    .acquire_owned()
                    .await
                    .map_err(|_| AppError::rpc("RPC semaphore closed"))?;
                let header = rpc.get_block(number).await?;
                Ok((number, header))
            });
        }

        let expected = (to_block - from_block + 1) as usize;
        let mut by_number: HashMap<u64, BlockHeader> = HashMap::with_capacity(expected);
        while let Some(joined) = join_set.join_next().await {
            match joined {
                Ok(Ok((number, Some(header)))) => {
                    by_number.insert(number, header);
                }
                Ok(Ok((number, None))) => {
                    warn!(
                        number,
                        "RPC returned no block header; possible mid-fetch reorg"
                    );
                    return Err(AppError::rpc(format!(
                        "block {number} disappeared while fetching a batch"
                    )));
                }
                Ok(Err(error)) => return Err(error),
                Err(join_error) => {
                    return Err(AppError::rpc(format!(
                        "header fetch task failed: {join_error}"
                    )));
                }
            }
        }

        let mut headers = Vec::with_capacity(expected);
        for number in from_block..=to_block {
            let header = by_number.remove(&number).ok_or_else(|| {
                AppError::rpc(format!(
                    "missing header for block {number} after concurrent fetch"
                ))
            })?;
            headers.push(header);
        }
        Ok(headers)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::RawLog;
    use crate::infrastructure::blockchain::decoder::ERC20_TRANSFER_TOPIC;
    use alloy_primitives::{Address, B256, U256};
    use async_trait::async_trait;
    use std::time::Duration;

    struct StaticRpc {
        headers: HashMap<u64, BlockHeader>,
        logs: Vec<RawLog>,
    }

    #[async_trait]
    impl ChainReader for StaticRpc {
        async fn chain_id(&self) -> Result<u64, AppError> {
            Ok(31337)
        }
        async fn latest_block_number(&self) -> Result<u64, AppError> {
            Ok(2)
        }
        async fn get_block(&self, number: u64) -> Result<Option<BlockHeader>, AppError> {
            Ok(self.headers.get(&number).cloned())
        }
        async fn get_logs(
            &self,
            from_block: u64,
            to_block: u64,
            _contract: Option<Address>,
        ) -> Result<Vec<RawLog>, AppError> {
            Ok(self
                .logs
                .iter()
                .filter(|log| log.block_number >= from_block && log.block_number <= to_block)
                .cloned()
                .collect())
        }
    }

    fn header(number: u64) -> BlockHeader {
        BlockHeader {
            number,
            hash: B256::repeat_byte(number as u8),
            parent_hash: B256::repeat_byte(number.saturating_sub(1) as u8),
            timestamp: 1_700_000_000 + number,
        }
    }

    #[tokio::test]
    async fn groups_out_of_order_fetches_into_ordered_batches() {
        let mut headers = HashMap::new();
        headers.insert(1, header(1));
        headers.insert(2, header(2));

        let mut from_topic = [0u8; 32];
        from_topic[31] = 1;
        let mut to_topic = [0u8; 32];
        to_topic[31] = 2;

        let log = RawLog {
            address: Address::repeat_byte(9),
            topics: vec![
                ERC20_TRANSFER_TOPIC,
                B256::from(from_topic),
                B256::from(to_topic),
            ],
            data: U256::from(42u64).to_be_bytes::<32>().to_vec(),
            block_number: 2,
            block_hash: B256::repeat_byte(2),
            transaction_hash: B256::repeat_byte(3),
            log_index: 0,
            removed: false,
        };

        let rpc = Arc::new(StaticRpc {
            headers,
            logs: vec![log],
        });
        let config = Arc::new(Config {
            database_url: "postgres://localhost/chainwatch".into(),
            rpc_url: "http://127.0.0.1:8545".into(),
            chain_id: 31337,
            start_block: 0,
            confirmation_depth: 0,
            poll_interval: Duration::from_millis(1000),
            max_concurrent_requests: 4,
            max_retries: 0,
            request_timeout: Duration::from_secs(1),
            batch_size: 10,
            contract_address: None,
            api_host: "127.0.0.1".into(),
            api_port: 8080,
            max_reorg_depth: 16,
        });
        let processor = Processor::new(rpc, config);
        let batches = processor.fetch_range(1, 2).await.expect("fetch");
        assert_eq!(batches.len(), 2);
        assert_eq!(batches[0].header.number, 1);
        assert!(batches[0].events.is_empty());
        assert_eq!(batches[1].header.number, 2);
        assert_eq!(batches[1].events.len(), 1);
        assert_eq!(batches[1].events[0].amount, U256::from(42u64));
    }
}
