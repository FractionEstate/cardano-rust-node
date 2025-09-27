//! REST API Implementation
//!
//! Provides HTTP REST API endpoints for querying blockchain data and
//! submitting transactions. Based on OpenAPI specification in contracts/api.yaml.

use std::sync::Arc;
use std::net::SocketAddr;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use serde::Deserialize;
use serde_json::Value;
use tower::ServiceBuilder;
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use tracing::{debug, error, info};

use crate::{ApiError, Result};

pub mod handlers;
pub mod types;

pub use handlers::*;
pub use types::*;

/// Configuration for the REST API server
#[derive(Debug, Clone)]
pub struct RestApiConfig {
    /// Server bind address
    pub bind_address: SocketAddr,
    /// Enable CORS
    pub enable_cors: bool,
    /// API version prefix
    pub api_version: String,
}

impl Default for RestApiConfig {
    fn default() -> Self {
        Self {
            bind_address: "127.0.0.1:3001".parse().unwrap(),
            enable_cors: true,
            api_version: "v1".to_string(),
        }
    }
}

/// REST API server state
#[derive(Clone)]
pub struct RestApiState {
    pub chain_handler: Arc<dyn ChainRestHandler>,
    pub transaction_handler: Arc<dyn TransactionRestHandler>,
}

/// REST API server
pub struct RestApiServer {
    config: RestApiConfig,
    state: RestApiState,
}

impl RestApiServer {
    /// Create a new REST API server
    pub fn new(config: RestApiConfig, state: RestApiState) -> Self {
        Self { config, state }
    }

    /// Build the router with all endpoints
    fn build_router(&self) -> Router {
        let api_routes = Router::new()
            // Chain endpoints
            .route("/chain/tip", get(get_chain_tip))
            .route("/blocks/:block_hash", get(get_block))
            .route("/transactions/:tx_id", get(get_transaction))
            .route("/transactions", post(submit_transaction))
            .route("/addresses/:address/utxos", get(get_address_utxos))
            // Additional endpoints
            .route("/protocol-parameters", get(get_protocol_parameters))
            .route("/stake-pools", get(get_stake_pools))
            .route("/node/status", get(get_node_status));

        let mut app = Router::new()
            .nest(&format!("/api/{}", self.config.api_version), api_routes)
            .with_state(self.state.clone());

        // Add middleware
        let service_builder = ServiceBuilder::new().layer(TraceLayer::new_for_http());

        if self.config.enable_cors {
            app = app.layer(service_builder.layer(CorsLayer::permissive()));
        } else {
            app = app.layer(service_builder);
        }

        app
    }

    /// Start the REST API server
    pub async fn serve(&self) -> Result<()> {
        let app = self.build_router();

        info!("REST API server starting on {}", self.config.bind_address);

        let listener = tokio::net::TcpListener::bind(&self.config.bind_address)
            .await
            .map_err(|e| ApiError::InternalError(format!("Failed to bind server: {}", e)))?;

        info!("REST API server listening on {}", self.config.bind_address);

        axum::serve(listener, app)
            .await
            .map_err(|e| ApiError::InternalError(format!("Server error: {}", e)))?;

        Ok(())
    }
}

/// Query parameters for address UTXOs
#[derive(Debug, Deserialize)]
pub struct UtxoQueryParams {
    /// Filter by specific asset (policy_id.asset_name)
    pub asset: Option<String>,
}

/// Chain tip endpoint
async fn get_chain_tip(State(state): State<RestApiState>) -> std::result::Result<Json<Value>, StatusCode> {
    debug!("GET /api/v1/chain/tip");

    match state.chain_handler.get_chain_tip().await {
        Ok(tip) => Ok(Json(tip)),
        Err(e) => {
            error!("Failed to get chain tip: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Get block by hash endpoint
async fn get_block(
    Path(block_hash): Path<String>,
    State(state): State<RestApiState>,
) -> std::result::Result<Json<Value>, StatusCode> {
    debug!("GET /api/v1/blocks/{}", block_hash);

    // Validate block hash format
    if block_hash.len() != 64 {
        return Err(StatusCode::BAD_REQUEST);
    }

    match state.chain_handler.get_block(&block_hash).await {
        Ok(Some(block)) => Ok(Json(block)),
        Ok(None) => Err(StatusCode::NOT_FOUND),
        Err(e) => {
            error!("Failed to get block {}: {}", block_hash, e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Get transaction by ID endpoint
async fn get_transaction(
    Path(tx_id): Path<String>,
    State(state): State<RestApiState>,
) -> std::result::Result<Json<Value>, StatusCode> {
    debug!("GET /api/v1/transactions/{}", tx_id);

    // Validate transaction ID format
    if tx_id.len() != 64 {
        return Err(StatusCode::BAD_REQUEST);
    }

    match state.chain_handler.get_transaction(&tx_id).await {
        Ok(Some(tx)) => Ok(Json(tx)),
        Ok(None) => Err(StatusCode::NOT_FOUND),
        Err(e) => {
            error!("Failed to get transaction {}: {}", tx_id, e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Submit transaction endpoint
async fn submit_transaction(
    State(state): State<RestApiState>,
    body: axum::body::Bytes,
) -> std::result::Result<Json<Value>, StatusCode> {
    debug!("POST /api/v1/transactions");

    // Try to parse as JSON first, then as CBOR
    let tx_data = if let Ok(json_str) = std::str::from_utf8(&body) {
        match serde_json::from_str::<Value>(json_str) {
            Ok(json_value) => json_value,
            Err(_) => {
                // Try as CBOR binary data
                serde_json::json!({
                    "cborData": hex::encode(&body)
                })
            }
        }
    } else {
        // Binary CBOR data
        serde_json::json!({
            "cborData": hex::encode(&body)
        })
    };

    match state.transaction_handler.submit_transaction(&tx_data).await {
        Ok(result) => Ok(Json(result)),
        Err(e) => {
            error!("Failed to submit transaction: {}", e);
            match e {
                ApiError::RequestError(_) => Err(StatusCode::BAD_REQUEST),
                _ => Err(StatusCode::INTERNAL_SERVER_ERROR),
            }
        }
    }
}

/// Get address UTXOs endpoint
async fn get_address_utxos(
    Path(address): Path<String>,
    Query(params): Query<UtxoQueryParams>,
    State(state): State<RestApiState>,
) -> std::result::Result<Json<Value>, StatusCode> {
    debug!("GET /api/v1/addresses/{}/utxos", address);

    match state.chain_handler.get_address_utxos(&address, params.asset.as_deref()).await {
        Ok(utxos) => Ok(Json(utxos)),
        Err(e) => {
            error!("Failed to get UTXOs for address {}: {}", address, e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Get protocol parameters endpoint
async fn get_protocol_parameters(
    State(state): State<RestApiState>,
) -> std::result::Result<Json<Value>, StatusCode> {
    debug!("GET /api/v1/protocol-parameters");

    match state.chain_handler.get_protocol_parameters().await {
        Ok(params) => Ok(Json(params)),
        Err(e) => {
            error!("Failed to get protocol parameters: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Get stake pools endpoint
async fn get_stake_pools(State(state): State<RestApiState>) -> std::result::Result<Json<Value>, StatusCode> {
    debug!("GET /api/v1/stake-pools");

    match state.chain_handler.get_stake_pools().await {
        Ok(pools) => Ok(Json(pools)),
        Err(e) => {
            error!("Failed to get stake pools: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Get node status endpoint
async fn get_node_status(State(state): State<RestApiState>) -> std::result::Result<Json<Value>, StatusCode> {
    debug!("GET /api/v1/node/status");

    match state.chain_handler.get_node_status().await {
        Ok(status) => Ok(Json(status)),
        Err(e) => {
            error!("Failed to get node status: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::util::ServiceExt;

    #[tokio::test]
    async fn test_router_creation() {
        let config = RestApiConfig::default();
        let state = RestApiState {
            chain_handler: Arc::new(MockChainRestHandler),
            transaction_handler: Arc::new(MockTransactionRestHandler),
        };

        let server = RestApiServer::new(config, state);
        let _router = server.build_router();
    }

    #[tokio::test]
    async fn test_chain_tip_endpoint() {
        let config = RestApiConfig::default();
        let state = RestApiState {
            chain_handler: Arc::new(MockChainRestHandler),
            transaction_handler: Arc::new(MockTransactionRestHandler),
        };

        let server = RestApiServer::new(config, state);
        let app = server.build_router();

        let request = Request::builder()
            .uri("/api/v1/chain/tip")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    struct MockChainRestHandler;

    #[async_trait::async_trait]
    impl ChainRestHandler for MockChainRestHandler {
        async fn get_chain_tip(&self) -> Result<Value> {
            Ok(serde_json::json!({
                "blockHash": "test_hash",
                "slotNo": 123456,
                "epochNo": 456
            }))
        }

        async fn get_block(&self, _block_hash: &str) -> Result<Option<Value>> {
            Ok(Some(serde_json::json!({
                "hash": "test_hash",
                "slot": 123456
            })))
        }

        async fn get_transaction(&self, _tx_id: &str) -> Result<Option<Value>> {
            Ok(Some(serde_json::json!({
                "id": "test_tx",
                "fee": 174593
            })))
        }

        async fn get_address_utxos(&self, _address: &str, _asset: Option<&str>) -> Result<Value> {
            Ok(serde_json::json!([]))
        }

        async fn get_protocol_parameters(&self) -> Result<Value> {
            Ok(serde_json::json!({}))
        }

        async fn get_stake_pools(&self) -> Result<Value> {
            Ok(serde_json::json!([]))
        }

        async fn get_node_status(&self) -> Result<Value> {
            Ok(serde_json::json!({}))
        }
    }

    struct MockTransactionRestHandler;

    #[async_trait::async_trait]
    impl TransactionRestHandler for MockTransactionRestHandler {
        async fn submit_transaction(&self, _tx_data: &Value) -> Result<Value> {
            Ok(serde_json::json!({
                "txId": "submitted_tx_123",
                "status": "accepted"
            }))
        }
    }
}
