use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub struct BlockRow {
    pub num: i64,
    pub hash: Vec<u8>,
    pub parent_hash: Vec<u8>,
    pub timestamp: DateTime<Utc>,
    pub timestamp_ms: i64,
    pub gas_limit: i64,
    pub gas_used: i64,
    pub miner: Vec<u8>,
    pub extra_data: Option<Vec<u8>>,
}

#[derive(Debug, Clone, Default)]
pub struct TxRow {
    pub block_num: i64,
    pub block_timestamp: DateTime<Utc>,
    pub idx: i32,
    pub hash: Vec<u8>,
    pub tx_type: i16,
    pub from: Vec<u8>,
    pub to: Option<Vec<u8>>,
    pub value: String,
    pub input: Vec<u8>,
    pub gas_limit: i64,
    pub max_fee_per_gas: String,
    pub max_priority_fee_per_gas: String,
    pub gas_used: Option<i64>,
    pub nonce_key: Vec<u8>,
    pub nonce: i64,
    pub fee_token: Option<Vec<u8>>,
    pub fee_payer: Option<Vec<u8>>,
    pub calls: Option<serde_json::Value>,
    pub call_count: i16,
    pub valid_before: Option<i64>,
    pub valid_after: Option<i64>,
    pub signature_type: Option<i16>,
}

#[derive(Debug, Clone, Default)]
pub struct LogRow {
    pub block_num: i64,
    pub block_timestamp: DateTime<Utc>,
    pub log_idx: i32,
    pub tx_idx: i32,
    pub tx_hash: Vec<u8>,
    pub address: Vec<u8>,
    pub selector: Option<Vec<u8>>,
    pub topics: Vec<Vec<u8>>,
    pub data: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncState {
    pub chain_id: u64,
    /// Remote chain head block number
    pub head_num: u64,
    /// Highest block synced (forward direction)
    pub synced_num: u64,
    /// Lowest block synced going backwards (None = not started, Some(0) = complete)
    pub backfill_num: Option<u64>,
}

impl Default for SyncState {
    fn default() -> Self {
        Self {
            chain_id: 0,
            head_num: 0,
            synced_num: 0,
            backfill_num: None,
        }
    }
}

impl SyncState {
    /// Returns true if backfill is complete (reached genesis)
    pub fn backfill_complete(&self) -> bool {
        self.backfill_num == Some(0)
    }

    /// Returns the number of blocks remaining to backfill
    pub fn backfill_remaining(&self) -> u64 {
        match self.backfill_num {
            None => self.synced_num, // Haven't started, everything below synced_num
            Some(0) => 0,            // Complete
            Some(n) => n,            // Blocks 0..n remain
        }
    }

    /// Returns total indexed block count (handles gaps)
    pub fn indexed_range(&self) -> (u64, u64) {
        let low = self.backfill_num.unwrap_or(self.synced_num);
        (low, self.synced_num)
    }
}
