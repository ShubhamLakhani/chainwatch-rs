use crate::application::indexing::Indexer;
use crate::application::ports::{ChainReader, EventStore};
use crate::bootstrap::config::Config;
use crate::bootstrap::shutdown;
use crate::domain::AppError;
use crate::infrastructure::blockchain::HttpRpcClient;
use crate::infrastructure::persistence::migrations::run_migrations;
use crate::infrastructure::persistence::{IndexerLock, Repository, connect_pool};
use crate::infrastructure::telemetry::Metrics;
use crate::infrastructure::telemetry::logging::init_tracing;
use crate::interfaces::http::{AppState, router};
use anyhow::{Context, Result};
use std::sync::Arc;
use tokio_util::sync::CancellationToken;
use tracing::info;

pub async fn run() -> Result<()> {
    init_tracing();
    dotenvy::dotenv().ok();

    let config = Arc::new(Config::from_env().context("invalid configuration")?);
    info!(summary = %config.log_summary(), "starting chainwatch-rs");

    let metrics = Arc::new(Metrics::new());

    let pool = connect_pool(&config.database_url)
        .await
        .context("failed to connect to PostgreSQL")?;

    run_migrations(&pool)
        .await
        .context("failed to run database migrations")?;

    let lock = IndexerLock::acquire(&pool, config.chain_id)
        .await
        .context("failed to acquire indexer advisory lock")?;

    let rpc = HttpRpcClient::new(
        config.rpc_url.clone(),
        config.max_retries,
        config.request_timeout,
        Arc::clone(&metrics),
    )?;
    let rpc: Arc<dyn ChainReader> = Arc::new(rpc);

    let reported_chain_id = rpc
        .chain_id()
        .await
        .context("failed to query RPC chain id")?;
    if reported_chain_id != config.chain_id {
        return Err(AppError::ChainIdMismatch {
            configured: config.chain_id,
            actual: reported_chain_id,
        }
        .into());
    }
    info!(chain_id = reported_chain_id, rpc_url = %config.rpc_url, "connected to EVM JSON-RPC");

    let repo = Repository::new(pool);
    if let Some(checkpoint) = repo
        .get_checkpoint(config.chain_id as i64)
        .await
        .context("failed to load checkpoint")?
    {
        metrics.set_indexed_block(checkpoint.last_indexed_block as u64);
        info!(
            last_indexed_block = checkpoint.last_indexed_block,
            last_indexed_block_hash = %checkpoint.last_indexed_block_hash,
            "resuming from persisted checkpoint"
        );
    } else {
        info!(
            start_block = config.start_block,
            "no checkpoint found; starting from START_BLOCK"
        );
    }

    let shutdown_token = CancellationToken::new();
    shutdown::spawn_signal_handler(shutdown_token.clone());

    let indexer = Indexer::new(
        Arc::clone(&config),
        Arc::clone(&rpc),
        Arc::new(repo.clone()),
        Arc::clone(&metrics),
        shutdown_token.clone(),
    );

    let state = AppState {
        config: Arc::clone(&config),
        store: Arc::new(repo),
        rpc,
        metrics,
    };
    let app = router(state);
    let bind_addr = config.api_bind_addr();
    let listener = tokio::net::TcpListener::bind(&bind_addr)
        .await
        .with_context(|| format!("failed to bind HTTP API on {bind_addr}"))?;
    info!(%bind_addr, "HTTP API listening");

    let api_shutdown = shutdown_token.clone();
    let api = tokio::spawn(async move {
        axum::serve(listener, app)
            .with_graceful_shutdown(async move {
                api_shutdown.cancelled().await;
            })
            .await
    });

    let indexer_task = tokio::spawn(async move { indexer.run().await });

    tokio::select! {
        result = indexer_task => {
            result.context("indexer task joined with an error")??;
        }
        result = api => {
            result.context("API task joined with an error")??;
        }
    }

    if let Err(error) = lock.release().await {
        tracing::warn!(%error, "failed to release advisory lock");
    }

    info!("chainwatch-rs stopped");
    Ok(())
}
