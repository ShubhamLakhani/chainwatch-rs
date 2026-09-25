#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use alloy_primitives::Address;
use async_trait::async_trait;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use chainwatch_rs::application::ports::{ChainReader, EventStore};
use chainwatch_rs::bootstrap::config::Config;
use chainwatch_rs::domain::{AppError, BlockHeader, RawLog};
use chainwatch_rs::infrastructure::persistence::Repository;
use chainwatch_rs::infrastructure::telemetry::Metrics;
use chainwatch_rs::interfaces::http::{AppState, router};
use http_body_util::BodyExt;
use sqlx::postgres::PgPoolOptions;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tower::ServiceExt;

use common::{cleanup_chain, test_header, test_transfer, unique_chain_id};

struct MapRpc {
    blocks: Arc<Mutex<HashMap<u64, BlockHeader>>>,
}

impl MapRpc {
    fn new(headers: Vec<BlockHeader>) -> Self {
        let mut map = HashMap::new();
        for header in headers {
            map.insert(header.number, header);
        }
        Self {
            blocks: Arc::new(Mutex::new(map)),
        }
    }
}

#[async_trait]
impl ChainReader for MapRpc {
    async fn chain_id(&self) -> Result<u64, AppError> {
        Ok(31337)
    }
    async fn latest_block_number(&self) -> Result<u64, AppError> {
        Ok(self.blocks.lock().await.keys().copied().max().unwrap_or(0))
    }
    async fn get_block(&self, number: u64) -> Result<Option<BlockHeader>, AppError> {
        Ok(self.blocks.lock().await.get(&number).cloned())
    }
    async fn get_logs(
        &self,
        _from: u64,
        _to: u64,
        _contract: Option<Address>,
    ) -> Result<Vec<RawLog>, AppError> {
        Ok(Vec::new())
    }
}

#[tokio::test]
#[ignore = "integration test: requires PostgreSQL (DATABASE_URL)"]
async fn health_and_events_api_responses() {
    let url = common::required_database_url();
    let pool = PgPoolOptions::new()
        .connect(&url)
        .await
        .expect("failed to connect to PostgreSQL");
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("migrate");
    let repo = Repository::new(pool.clone());
    let chain_id = unique_chain_id();
    let header = test_header(4, 0x44);
    repo.persist_block(chain_id, &header, &[test_transfer(&header, 0, 55)])
        .await
        .unwrap();

    let config = Arc::new(Config {
        database_url: url.clone(),
        rpc_url: "http://127.0.0.1:8545".into(),
        chain_id: chain_id as u64,
        start_block: 0,
        confirmation_depth: 0,
        poll_interval: Duration::from_millis(1000),
        max_concurrent_requests: 2,
        max_retries: 1,
        request_timeout: Duration::from_secs(2),
        batch_size: 10,
        contract_address: None,
        api_host: "127.0.0.1".into(),
        api_port: 0,
        max_reorg_depth: 16,
    });
    let rpc: Arc<dyn ChainReader> = Arc::new(MapRpc::new(vec![header.clone()]));
    let app = router(AppState {
        config,
        store: Arc::new(repo),
        rpc,
        metrics: Arc::new(Metrics::new()),
    });

    let health = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(health.status(), StatusCode::OK);

    let events = app
        .oneshot(
            Request::builder()
                .uri("/events")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(events.status(), StatusCode::OK);
    let bytes = events.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert!(!json["events"].as_array().unwrap().is_empty());

    cleanup_chain(&pool, chain_id).await;
}
