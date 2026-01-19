use metrics::{counter, gauge, histogram};
use std::time::Instant;

pub fn record_blocks_indexed(count: u64) {
    counter!("ak47_blocks_indexed_total").increment(count);
}

pub fn record_txs_indexed(count: u64) {
    counter!("ak47_txs_indexed_total").increment(count);
}

pub fn record_logs_indexed(count: u64) {
    counter!("ak47_logs_indexed_total").increment(count);
}

pub fn set_sync_head(block_num: u64) {
    gauge!("ak47_sync_head_block").set(block_num as f64);
}

pub fn set_sync_lag(lag: u64) {
    gauge!("ak47_sync_lag_blocks").set(lag as f64);
}

pub fn set_backfill_progress(block_num: u64) {
    gauge!("ak47_backfill_block").set(block_num as f64);
}

pub fn record_rpc_request(method: &str, duration: std::time::Duration, success: bool) {
    let labels = [("method", method.to_string()), ("success", success.to_string())];
    counter!("ak47_rpc_requests_total", &labels).increment(1);
    histogram!("ak47_rpc_request_duration_seconds", &labels).record(duration.as_secs_f64());
}

pub fn record_query_duration(duration: std::time::Duration) {
    histogram!("ak47_query_duration_seconds").record(duration.as_secs_f64());
}

pub fn record_query_rows(count: u64) {
    histogram!("ak47_query_rows").record(count as f64);
}

pub struct Timer {
    start: Instant,
}

impl Timer {
    pub fn start() -> Self {
        Self {
            start: Instant::now(),
        }
    }

    pub fn elapsed(&self) -> std::time::Duration {
        self.start.elapsed()
    }
}
