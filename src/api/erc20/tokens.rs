//! ERC20 token list endpoint, backed by the `erc20_tokens` table.
//!
//! The table is populated by `src/sync/erc20_metadata.rs`; this module only
//! reads. Exposes `GET /erc20/tokens` which returns the N newest discovered
//! tokens.

use std::time::Instant;

use axum::{
    extract::{Query, State},
    Json,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::api::{ApiError, AppState};

#[derive(Deserialize)]
pub struct TokensParams {
    #[serde(alias = "chain_id", rename = "chainId")]
    chain_id: u64,
}

#[derive(Serialize)]
pub struct Erc20Token {
    contract_address: String,
    name: Option<String>,
    symbol: Option<String>,
    decimals: Option<i16>,
    first_transfer_at: String,
    first_transfer_block: i64,
    deployed_at: Option<String>,
    deployed_block: Option<i64>,
    resolution_status: String,
    /// Block timestamp the metadata was read at (via
    /// Multicall3.getCurrentBlockTimestamp()). Null if never resolved or
    /// the block-timestamp sub-call reverted.
    resolved_at: Option<String>,
    /// Block number paired with `resolved_at` (via Multicall3.getBlockNumber()).
    resolved_block: Option<i64>,
}

#[derive(Serialize)]
pub struct Erc20TokensResponse {
    ok: bool,
    tokens: Vec<Erc20Token>,
    count: usize,
    total_count: i64,
    query_time_ms: f64,
}

/// Hard cap on how many tokens the list endpoint returns. Without pagination
/// we don't want to stream the full `erc20_tokens` table (can be hundreds of
/// thousands of rows on mainnet).
const LIST_LIMIT: i64 = 100;

/// GET /erc20/tokens?chainId=X
///
/// Returns the [`LIST_LIMIT`] most recently discovered ERC20 tokens.
pub async fn list_tokens(
    State(state): State<AppState>,
    Query(params): Query<TokensParams>,
) -> Result<Json<Erc20TokensResponse>, ApiError> {
    let pool = state
        .get_pool(Some(params.chain_id))
        .await
        .ok_or_else(|| ApiError::BadRequest(format!("Unknown chainId: {}", params.chain_id)))?;
    let conn = pool
        .get()
        .await
        .map_err(|e| ApiError::Internal(format!("Pool error: {e}")))?;

    let start = Instant::now();

    let rows = conn
        .query(
            r#"
            SELECT address, name, symbol, decimals,
                   first_transfer_at, first_transfer_block,
                   deployed_at, deployed_block,
                   resolution_status, resolved_at, resolved_block
            FROM erc20_tokens
            ORDER BY first_transfer_at DESC
            LIMIT $1
            "#,
            &[&LIST_LIMIT],
        )
        .await
        .map_err(|e| ApiError::QueryError(e.to_string()))?;

    // Total count is a small index-backed aggregate; fine to run per request.
    let total_count: i64 = conn
        .query_one("SELECT COUNT(*) FROM erc20_tokens", &[])
        .await
        .map_err(|e| ApiError::QueryError(e.to_string()))?
        .get(0);

    let tokens: Vec<Erc20Token> = rows.iter().map(row_to_token).collect();
    let count = tokens.len();
    let query_time_ms = start.elapsed().as_secs_f64() * 1000.0;

    Ok(Json(Erc20TokensResponse {
        ok: true,
        tokens,
        count,
        total_count,
        query_time_ms,
    }))
}

fn row_to_token(row: &tokio_postgres::Row) -> Erc20Token {
    let address: Vec<u8> = row.get(0);
    let first_transfer_at: DateTime<Utc> = row.get(4);
    let deployed_at: Option<DateTime<Utc>> = row.get(6);
    let resolved_at: Option<DateTime<Utc>> = row.get(9);
    Erc20Token {
        contract_address: format!("0x{}", hex::encode(&address)),
        name: row.get(1),
        symbol: row.get(2),
        decimals: row.get(3),
        first_transfer_at: first_transfer_at.to_rfc3339(),
        first_transfer_block: row.get(5),
        deployed_at: deployed_at.map(|d| d.to_rfc3339()),
        deployed_block: row.get(7),
        resolution_status: row.get(8),
        resolved_at: resolved_at.map(|d| d.to_rfc3339()),
        resolved_block: row.get(10),
    }
}
