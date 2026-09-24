use serde::Serialize;
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Debug, Default)]
pub struct Metrics {
    indexed_block: AtomicU64,
    latest_block: AtomicU64,
    total_indexed_events: AtomicU64,
    rpc_request_failures: AtomicU64,
    retry_count: AtomicU64,
    reorg_count: AtomicU64,
    duplicate_events: AtomicU64,
    database_errors: AtomicU64,
}

#[derive(Debug, Clone, Serialize)]
pub struct MetricsSnapshot {
    pub indexed_block: u64,
    pub latest_observed_block: u64,
    pub indexing_lag: u64,
    pub total_indexed_events: u64,
    pub rpc_request_failures: u64,
    pub retry_count: u64,
    pub reorg_count: u64,
    pub duplicate_events: u64,
    pub database_errors: u64,
}

impl Metrics {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_indexed_block(&self, block: u64) {
        self.indexed_block.store(block, Ordering::Relaxed);
    }

    pub fn set_latest_block(&self, block: u64) {
        self.latest_block.store(block, Ordering::Relaxed);
    }

    pub fn add_indexed_events(&self, count: u64) {
        self.total_indexed_events
            .fetch_add(count, Ordering::Relaxed);
    }

    pub fn add_duplicates(&self, count: u64) {
        self.duplicate_events.fetch_add(count, Ordering::Relaxed);
    }

    pub fn incr_rpc_failure(&self) {
        self.rpc_request_failures.fetch_add(1, Ordering::Relaxed);
    }

    pub fn incr_retry(&self) {
        self.retry_count.fetch_add(1, Ordering::Relaxed);
    }

    pub fn incr_reorg(&self) {
        self.reorg_count.fetch_add(1, Ordering::Relaxed);
    }

    pub fn incr_database_error(&self) {
        self.database_errors.fetch_add(1, Ordering::Relaxed);
    }

    pub fn snapshot(&self) -> MetricsSnapshot {
        let indexed = self.indexed_block.load(Ordering::Relaxed);
        let latest = self.latest_block.load(Ordering::Relaxed);
        MetricsSnapshot {
            indexed_block: indexed,
            latest_observed_block: latest,
            indexing_lag: latest.saturating_sub(indexed),
            total_indexed_events: self.total_indexed_events.load(Ordering::Relaxed),
            rpc_request_failures: self.rpc_request_failures.load(Ordering::Relaxed),
            retry_count: self.retry_count.load(Ordering::Relaxed),
            reorg_count: self.reorg_count.load(Ordering::Relaxed),
            duplicate_events: self.duplicate_events.load(Ordering::Relaxed),
            database_errors: self.database_errors.load(Ordering::Relaxed),
        }
    }

    pub fn prometheus(&self) -> String {
        let s = self.snapshot();
        format!(
            "# HELP chainwatch_indexed_block Last successfully persisted block.\n\
             # TYPE chainwatch_indexed_block gauge\n\
             chainwatch_indexed_block {}\n\
             # HELP chainwatch_latest_observed_block Latest block number reported by RPC.\n\
             # TYPE chainwatch_latest_observed_block gauge\n\
             chainwatch_latest_observed_block {}\n\
             # HELP chainwatch_indexing_lag Difference between latest observed and indexed block.\n\
             # TYPE chainwatch_indexing_lag gauge\n\
             chainwatch_indexing_lag {}\n\
             # HELP chainwatch_indexed_events_total Events inserted (excluding duplicates).\n\
             # TYPE chainwatch_indexed_events_total counter\n\
             chainwatch_indexed_events_total {}\n\
             # HELP chainwatch_rpc_request_failures_total Failed RPC attempts.\n\
             # TYPE chainwatch_rpc_request_failures_total counter\n\
             chainwatch_rpc_request_failures_total {}\n\
             # HELP chainwatch_retry_count_total Retry attempts after transient failures.\n\
             # TYPE chainwatch_retry_count_total counter\n\
             chainwatch_retry_count_total {}\n\
             # HELP chainwatch_reorg_count_total Detected chain reorganizations.\n\
             # TYPE chainwatch_reorg_count_total counter\n\
             chainwatch_reorg_count_total {}\n\
             # HELP chainwatch_duplicate_events_total Duplicate events ignored via ON CONFLICT.\n\
             # TYPE chainwatch_duplicate_events_total counter\n\
             chainwatch_duplicate_events_total {}\n\
             # HELP chainwatch_database_errors_total Failed PostgreSQL operations.\n\
             # TYPE chainwatch_database_errors_total counter\n\
             chainwatch_database_errors_total {}\n",
            s.indexed_block,
            s.latest_observed_block,
            s.indexing_lag,
            s.total_indexed_events,
            s.rpc_request_failures,
            s.retry_count,
            s.reorg_count,
            s.duplicate_events,
            s.database_errors
        )
    }
}
