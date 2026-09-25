//! Main indexing loop: poll the chain tip, handle reorgs, fetch, persist.

use crate::application::indexing::pipeline::Processor;
use crate::application::indexing::reorg::{ReorgAction, detect_reorg, parent_hash_mismatch};
use crate::application::ports::{ChainReader, EventStore};
use crate::bootstrap::config::Config;
use crate::domain::{AppError, next_block_to_index, safe_head};
use crate::infrastructure::telemetry::metrics::Metrics;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, warn};

pub struct Indexer {
    config: Arc<Config>,
    rpc: Arc<dyn ChainReader>,
    repo: Arc<dyn EventStore>,
    processor: Processor,
    metrics: Arc<Metrics>,
    shutdown: CancellationToken,
}

impl Indexer {
    pub fn new(
        config: Arc<Config>,
        rpc: Arc<dyn ChainReader>,
        repo: Arc<dyn EventStore>,
        metrics: Arc<Metrics>,
        shutdown: CancellationToken,
    ) -> Self {
        let processor = Processor::new(Arc::clone(&rpc), Arc::clone(&config));
        Self {
            config,
            rpc,
            repo,
            processor,
            metrics,
            shutdown,
        }
    }

    pub async fn run(&self) -> Result<(), AppError> {
        let mut interval = tokio::time::interval(self.config.poll_interval);
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

        loop {
            tokio::select! {
                _ = self.shutdown.cancelled() => {
                    info!("indexer loop received shutdown");
                    return Ok(());
                }
                _ = interval.tick() => {
                    if let Err(error) = self.sync_once().await {
                        warn!(%error, "indexing tick failed; will retry on the next poll");
                    }
                }
            }
        }
    }

    pub async fn sync_once(&self) -> Result<(), AppError> {
        let latest = self.rpc.latest_block_number().await?;
        self.metrics.set_latest_block(latest);

        match detect_reorg(
            self.rpc.as_ref(),
            self.repo.as_ref(),
            self.config.chain_id as i64,
            self.config.max_reorg_depth,
        )
        .await?
        {
            ReorgAction::None => {}
            ReorgAction::Rollback(plan) => {
                self.metrics.incr_reorg();
                info!(
                    diverged_at = plan.diverged_at,
                    ancestor = plan.ancestor_block,
                    "applying reorganization rollback"
                );
                self.repo
                    .rollback_after(
                        self.config.chain_id as i64,
                        plan.ancestor_block,
                        &plan.ancestor_hash,
                    )
                    .await?;
                self.metrics.set_indexed_block(plan.ancestor_block);
            }
            ReorgAction::ResetChain => {
                self.metrics.incr_reorg();
                warn!(
                    "resetting indexed data after genesis mismatch (new chain under the same CHAIN_ID)"
                );
                self.repo.reset_chain(self.config.chain_id as i64).await?;
                self.metrics.set_indexed_block(0);
            }
        }

        let checkpoint = self
            .repo
            .get_checkpoint(self.config.chain_id as i64)
            .await?;
        if let Some(cp) = &checkpoint {
            self.metrics.set_indexed_block(cp.last_indexed_block as u64);
        }

        let Some(safe) = safe_head(latest, self.config.confirmation_depth) else {
            debug!(
                latest,
                depth = self.config.confirmation_depth,
                "waiting for confirmation depth before indexing"
            );
            return Ok(());
        };

        let start = next_block_to_index(
            checkpoint.map(|cp| cp.last_indexed_block as u64),
            self.config.start_block,
        );
        if start > safe {
            debug!(start, safe, latest, "indexer is caught up");
            return Ok(());
        }

        info!(from = start, to = safe, latest, "indexing block range");

        let mut from = start;
        while from <= safe {
            if self.shutdown.is_cancelled() {
                return Ok(());
            }
            let to = (from + self.config.batch_size - 1).min(safe);
            self.persist_range(from, to).await?;
            from = to + 1;
        }

        Ok(())
    }

    async fn persist_range(&self, from: u64, to: u64) -> Result<(), AppError> {
        let batches = self.processor.fetch_range(from, to).await?;
        let mut previous = if from > 0 {
            self.rpc.get_block(from - 1).await?
        } else {
            None
        };

        for batch in batches {
            if self.shutdown.is_cancelled() {
                return Ok(());
            }
            if parent_hash_mismatch(previous.as_ref(), &batch.header) {
                warn!(
                    block = batch.header.number,
                    parent = %batch.header.parent_hash_hex(),
                    "parent hash does not connect to the previous block; aborting batch for reorg handling"
                );
                return Ok(());
            }

            let outcome = match self
                .repo
                .persist_block(self.config.chain_id as i64, &batch.header, &batch.events)
                .await
            {
                Ok(outcome) => outcome,
                Err(error) => {
                    self.metrics.incr_database_error();
                    return Err(error);
                }
            };
            self.metrics.set_indexed_block(batch.header.number);
            self.metrics.add_indexed_events(outcome.inserted_events);
            self.metrics.add_duplicates(outcome.duplicate_events);

            if !batch.events.is_empty() {
                info!(
                    block = batch.header.number,
                    events = batch.events.len(),
                    inserted = outcome.inserted_events,
                    duplicates = outcome.duplicate_events,
                    "persisted block"
                );
            } else {
                debug!(block = batch.header.number, "persisted empty block");
            }

            previous = Some(batch.header);
        }

        Ok(())
    }
}
