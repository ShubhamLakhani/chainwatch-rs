use crate::domain::safe_head;
use crate::domain::{EventListFilter, is_address, is_tx_hash, normalize_hex};
use crate::interfaces::http::dto::{
    DEFAULT_PAGE_LIMIT, EventDto, EventListResponse, EventQueryParams, HealthResponse,
    MAX_PAGE_LIMIT, ReadyResponse, StatusResponse,
};
use crate::interfaces::http::error::ApiError;
use crate::interfaces::http::state::AppState;
use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};

pub async fn health() -> Json<HealthResponse> {
    Json(HealthResponse { status: "ok" })
}

pub async fn ready(State(state): State<AppState>) -> Response {
    let database = state.store.ping().await.is_ok();
    let rpc = state.rpc.latest_block_number().await.is_ok();
    let ok = database && rpc;
    let body = ReadyResponse {
        status: if ok { "ready" } else { "not_ready" },
        database,
        rpc,
    };
    let status = if ok {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };
    (status, Json(body)).into_response()
}

pub async fn status(State(state): State<AppState>) -> Result<Json<StatusResponse>, ApiError> {
    let checkpoint = state
        .store
        .get_checkpoint(state.config.chain_id as i64)
        .await
        .map_err(ApiError::internal)?;
    let snapshot = state.metrics.snapshot();
    let last_indexed = checkpoint.as_ref().map(|cp| cp.last_indexed_block);
    let safe = safe_head(
        snapshot.latest_observed_block,
        state.config.confirmation_depth,
    );
    let catch_up_lag = match (safe, last_indexed) {
        (Some(safe_block), Some(indexed)) => safe_block.saturating_sub(indexed as u64),
        (Some(safe_block), None) => safe_block.saturating_sub(state.config.start_block),
        _ => 0,
    };

    Ok(Json(StatusResponse {
        chain_id: state.config.chain_id,
        confirmation_depth: state.config.confirmation_depth,
        last_indexed_block: last_indexed,
        last_indexed_block_hash: checkpoint.map(|cp| cp.last_indexed_block_hash),
        latest_observed_block: snapshot.latest_observed_block,
        safe_head: safe,
        indexing_lag: snapshot.indexing_lag,
        catch_up_lag,
        total_indexed_events: snapshot.total_indexed_events,
        rpc_request_failures: snapshot.rpc_request_failures,
        retry_count: snapshot.retry_count,
        reorg_count: snapshot.reorg_count,
        contract_address: state
            .config
            .contract_address
            .map(|addr| format!("{addr:#x}")),
    }))
}

pub async fn list_events(
    State(state): State<AppState>,
    Query(query): Query<EventQueryParams>,
) -> Result<Json<EventListResponse>, ApiError> {
    let limit = query.limit.unwrap_or(DEFAULT_PAGE_LIMIT);
    if limit == 0 || limit > MAX_PAGE_LIMIT {
        return Err(ApiError::bad_request(format!(
            "limit must be between 1 and {MAX_PAGE_LIMIT}"
        )));
    }
    let offset = query.offset.unwrap_or(0);

    let address = match query.address {
        Some(value) => {
            if !is_address(&value) {
                return Err(ApiError::bad_request(
                    "address must be a 20-byte hex string",
                ));
            }
            Some(normalize_hex(&value))
        }
        None => None,
    };
    let contract = match query.contract {
        Some(value) => {
            if !is_address(&value) {
                return Err(ApiError::bad_request(
                    "contract must be a 20-byte hex string",
                ));
            }
            Some(normalize_hex(&value))
        }
        None => None,
    };

    let events = state
        .store
        .list_events(EventListFilter {
            chain_id: state.config.chain_id as i64,
            address,
            contract,
            limit: i64::from(limit),
            offset: i64::from(offset),
        })
        .await
        .map_err(ApiError::internal)?;

    Ok(Json(EventListResponse {
        events: events.into_iter().map(EventDto::from).collect(),
        limit: i64::from(limit),
        offset: i64::from(offset),
    }))
}

pub async fn events_by_tx(
    State(state): State<AppState>,
    Path(transaction_hash): Path<String>,
) -> Result<Json<EventListResponse>, ApiError> {
    if !is_tx_hash(&transaction_hash) {
        return Err(ApiError::bad_request(
            "transaction hash must be a 32-byte hex string",
        ));
    }
    let hash = normalize_hex(&transaction_hash);
    let events = state
        .store
        .events_by_tx(state.config.chain_id as i64, &hash)
        .await
        .map_err(ApiError::internal)?;
    if events.is_empty() {
        return Err(ApiError::not_found("no events for that transaction"));
    }
    Ok(Json(EventListResponse {
        limit: events.len() as i64,
        offset: 0,
        events: events.into_iter().map(EventDto::from).collect(),
    }))
}

pub async fn metrics_handler(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let accept = headers
        .get(header::ACCEPT)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    if accept.contains("text/plain") {
        let mut response = Response::new(state.metrics.prometheus().into());
        *response.status_mut() = StatusCode::OK;
        response.headers_mut().insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("text/plain; version=0.0.4"),
        );
        response
    } else {
        Json(state.metrics.snapshot()).into_response()
    }
}
