use std::sync::Arc;

use anyhow::Result;
use tokio::sync::mpsc;

use crate::db::{DuckDbPool, Pool};
use crate::types::{BlockRow, LogRow, TxRow};

/// Batch of rows to replicate to DuckDB.
#[derive(Debug)]
pub enum ReplicaBatch {
    Blocks(Vec<BlockRow>),
    Txs(Vec<TxRow>),
    Logs(Vec<LogRow>),
}

/// DuckDB replicator that syncs data from PostgreSQL.
///
/// Uses a channel-based approach where the indexer sends batches
/// and the replicator flushes them to DuckDB in micro-batches.
pub struct Replicator {
    duckdb: Arc<DuckDbPool>,
    rx: mpsc::Receiver<ReplicaBatch>,
}

/// Handle for sending batches to the replicator.
#[derive(Clone)]
pub struct ReplicatorHandle {
    tx: mpsc::Sender<ReplicaBatch>,
}

impl ReplicatorHandle {
    /// Sends a batch of blocks to be replicated.
    pub async fn send_blocks(&self, blocks: Vec<BlockRow>) -> Result<()> {
        if blocks.is_empty() {
            return Ok(());
        }
        self.tx
            .send(ReplicaBatch::Blocks(blocks))
            .await
            .map_err(|_| anyhow::anyhow!("Replicator channel closed"))
    }

    /// Sends a batch of transactions to be replicated.
    pub async fn send_txs(&self, txs: Vec<TxRow>) -> Result<()> {
        if txs.is_empty() {
            return Ok(());
        }
        self.tx
            .send(ReplicaBatch::Txs(txs))
            .await
            .map_err(|_| anyhow::anyhow!("Replicator channel closed"))
    }

    /// Sends a batch of logs to be replicated.
    pub async fn send_logs(&self, logs: Vec<LogRow>) -> Result<()> {
        if logs.is_empty() {
            return Ok(());
        }
        self.tx
            .send(ReplicaBatch::Logs(logs))
            .await
            .map_err(|_| anyhow::anyhow!("Replicator channel closed"))
    }
}

impl Replicator {
    /// Creates a new replicator with a channel for receiving batches.
    pub fn new(duckdb: Arc<DuckDbPool>, buffer_size: usize) -> (Self, ReplicatorHandle) {
        let (tx, rx) = mpsc::channel(buffer_size);
        (Self { duckdb, rx }, ReplicatorHandle { tx })
    }

    /// Runs the replicator, processing batches as they arrive.
    pub async fn run(mut self) -> Result<()> {
        tracing::info!("DuckDB replicator started");

        while let Some(batch) = self.rx.recv().await {
            if let Err(e) = self.process_batch(batch).await {
                tracing::error!(error = %e, "Failed to replicate batch to DuckDB");
            }
        }

        tracing::info!("DuckDB replicator stopped");
        Ok(())
    }

    async fn process_batch(&self, batch: ReplicaBatch) -> Result<()> {
        let conn = self.duckdb.conn().await;

        match batch {
            ReplicaBatch::Blocks(blocks) => {
                let count = blocks.len();
                for block in blocks {
                    conn.execute(
                        "INSERT OR REPLACE INTO blocks (num, hash, parent_hash, timestamp, timestamp_ms, gas_limit, gas_used, miner, extra_data)
                         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
                        duckdb::params![
                            block.num,
                            format!("0x{}", hex::encode(&block.hash)),
                            format!("0x{}", hex::encode(&block.parent_hash)),
                            block.timestamp.to_rfc3339(),
                            block.timestamp_ms,
                            block.gas_limit,
                            block.gas_used,
                            format!("0x{}", hex::encode(&block.miner)),
                            block.extra_data.as_ref().map(|d| format!("0x{}", hex::encode(d))),
                        ],
                    )?;
                }
                Self::update_watermark(&conn)?;
                tracing::debug!(count, "Replicated blocks to DuckDB");
            }
            ReplicaBatch::Txs(txs) => {
                let count = txs.len();
                for tx in txs {
                    conn.execute(
                        "INSERT OR REPLACE INTO txs (block_num, block_timestamp, idx, hash, type, \"from\", \"to\", value, input, gas_limit, max_fee_per_gas, max_priority_fee_per_gas, gas_used, nonce_key, nonce, fee_token, fee_payer, calls, call_count, valid_before, valid_after, signature_type)
                         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                        duckdb::params![
                            tx.block_num,
                            tx.block_timestamp.to_rfc3339(),
                            tx.idx,
                            format!("0x{}", hex::encode(&tx.hash)),
                            tx.tx_type,
                            format!("0x{}", hex::encode(&tx.from)),
                            tx.to.as_ref().map(|t| format!("0x{}", hex::encode(t))),
                            tx.value,
                            format!("0x{}", hex::encode(&tx.input)),
                            tx.gas_limit,
                            tx.max_fee_per_gas,
                            tx.max_priority_fee_per_gas,
                            tx.gas_used,
                            format!("0x{}", hex::encode(&tx.nonce_key)),
                            tx.nonce,
                            tx.fee_token.as_ref().map(|t| format!("0x{}", hex::encode(t))),
                            tx.fee_payer.as_ref().map(|t| format!("0x{}", hex::encode(t))),
                            tx.calls.as_ref().map(|c| c.to_string()),
                            tx.call_count,
                            tx.valid_before,
                            tx.valid_after,
                            tx.signature_type,
                        ],
                    )?;
                }
                Self::update_watermark(&conn)?;
                tracing::debug!(count, "Replicated txs to DuckDB");
            }
            ReplicaBatch::Logs(logs) => {
                let count = logs.len();
                for log in logs {
                    let topics_str = format!(
                        "[{}]",
                        log.topics
                            .iter()
                            .map(|t| format!("'{}'", format!("0x{}", hex::encode(t))))
                            .collect::<Vec<_>>()
                            .join(", ")
                    );
                    conn.execute(
                        &format!(
                            "INSERT OR REPLACE INTO logs (block_num, block_timestamp, log_idx, tx_idx, tx_hash, address, selector, topics, data)
                             VALUES (?, ?, ?, ?, ?, ?, ?, {}, ?)",
                            topics_str
                        ),
                        duckdb::params![
                            log.block_num,
                            log.block_timestamp.to_rfc3339(),
                            log.log_idx,
                            log.tx_idx,
                            format!("0x{}", hex::encode(&log.tx_hash)),
                            format!("0x{}", hex::encode(&log.address)),
                            log.selector.as_ref().map(|s| format!("0x{}", hex::encode(s))),
                            format!("0x{}", hex::encode(&log.data)),
                        ],
                    )?;
                }
                Self::update_watermark(&conn)?;
                tracing::debug!(count, "Replicated logs to DuckDB");
            }
        }

        Ok(())
    }

    fn update_watermark(conn: &duckdb::Connection) -> Result<()> {
        conn.execute(
            "UPDATE duckdb_sync_state SET 
                latest_block = (SELECT COALESCE(MAX(num), 0) FROM blocks),
                updated_at = CURRENT_TIMESTAMP
             WHERE id = 1",
            [],
        )?;
        Ok(())
    }
}

/// Backfills DuckDB from PostgreSQL for blocks that haven't been synced yet.
pub async fn backfill_from_postgres(
    pg_pool: &Pool,
    duckdb: &Arc<DuckDbPool>,
    batch_size: i64,
) -> Result<u64> {
    // Get current DuckDB watermark
    let duckdb_latest = duckdb.latest_block().await?.unwrap_or(0);

    // Get PostgreSQL latest block
    let pg_conn = pg_pool.get().await?;
    let pg_latest: i64 = pg_conn
        .query_one("SELECT COALESCE(MAX(num), 0) FROM blocks", &[])
        .await?
        .get(0);

    if duckdb_latest >= pg_latest {
        tracing::info!(
            duckdb_latest,
            pg_latest,
            "DuckDB is up to date with PostgreSQL"
        );
        return Ok(0);
    }

    let blocks_to_sync = pg_latest - duckdb_latest;
    tracing::info!(
        duckdb_latest,
        pg_latest,
        blocks_to_sync,
        "Starting DuckDB backfill from PostgreSQL"
    );

    let mut synced = 0u64;
    let mut current = duckdb_latest + 1;

    while current <= pg_latest {
        let end = (current + batch_size - 1).min(pg_latest);

        // Fetch blocks from PostgreSQL
        let rows = pg_conn
            .query(
                "SELECT num, hash, parent_hash, timestamp, timestamp_ms, gas_limit, gas_used, miner, extra_data 
                 FROM blocks WHERE num >= $1 AND num <= $2 ORDER BY num",
                &[&current, &end],
            )
            .await?;

        // Insert into DuckDB
        let duck_conn = duckdb.conn().await;
        for row in &rows {
            let num: i64 = row.get(0);
            let hash: Vec<u8> = row.get(1);
            let parent_hash: Vec<u8> = row.get(2);
            let timestamp: chrono::DateTime<chrono::Utc> = row.get(3);
            let timestamp_ms: i64 = row.get(4);
            let gas_limit: i64 = row.get(5);
            let gas_used: i64 = row.get(6);
            let miner: Vec<u8> = row.get(7);
            let extra_data: Option<Vec<u8>> = row.get(8);

            duck_conn.execute(
                "INSERT OR REPLACE INTO blocks (num, hash, parent_hash, timestamp, timestamp_ms, gas_limit, gas_used, miner, extra_data)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
                duckdb::params![
                    num,
                    format!("0x{}", hex::encode(&hash)),
                    format!("0x{}", hex::encode(&parent_hash)),
                    timestamp.to_rfc3339(),
                    timestamp_ms,
                    gas_limit,
                    gas_used,
                    format!("0x{}", hex::encode(&miner)),
                    extra_data.as_ref().map(|d| format!("0x{}", hex::encode(d))),
                ],
            )?;
        }

        synced += rows.len() as u64;
        current = end + 1;

        if synced % 10000 == 0 {
            tracing::info!(synced, current, pg_latest, "DuckDB backfill progress");
        }
    }

    // Update watermark
    let duck_conn = duckdb.conn().await;
    duck_conn.execute(
        "UPDATE duckdb_sync_state SET latest_block = ?, updated_at = CURRENT_TIMESTAMP WHERE id = 1",
        duckdb::params![pg_latest],
    )?;

    tracing::info!(synced, "DuckDB backfill complete");
    Ok(synced)
}

/// Gets the current DuckDB sync status.
pub async fn get_sync_status(duckdb: &Arc<DuckDbPool>) -> Result<DuckDbSyncStatus> {
    let conn = duckdb.conn().await;
    let mut stmt = conn.prepare(
        "SELECT latest_block, updated_at FROM duckdb_sync_state WHERE id = 1",
    )?;

    let result = stmt.query_row([], |row| {
        Ok(DuckDbSyncStatus {
            latest_block: row.get(0)?,
            updated_at: row.get::<_, String>(1)?,
        })
    });

    match result {
        Ok(status) => Ok(status),
        Err(_) => Ok(DuckDbSyncStatus {
            latest_block: 0,
            updated_at: String::new(),
        }),
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct DuckDbSyncStatus {
    pub latest_block: i64,
    pub updated_at: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use std::time::Duration;

    #[tokio::test]
    async fn test_replicator_blocks() {
        let duckdb = Arc::new(DuckDbPool::in_memory().unwrap());
        let (replicator, handle) = Replicator::new(duckdb.clone(), 100);

        // Spawn replicator
        let replicator_task = tokio::spawn(replicator.run());

        // Send a block
        let block = BlockRow {
            num: 1,
            hash: vec![0xab; 32],
            parent_hash: vec![0x00; 32],
            timestamp: Utc::now(),
            timestamp_ms: 1704067200000,
            gas_limit: 30_000_000,
            gas_used: 21000,
            miner: vec![0xde; 20],
            extra_data: None,
        };

        handle.send_blocks(vec![block]).await.unwrap();

        // Give it time to process
        tokio::time::sleep(Duration::from_millis(50)).await;

        // Verify block was inserted
        let conn = duckdb.conn().await;
        let mut stmt = conn.prepare("SELECT num FROM blocks WHERE num = 1").unwrap();
        let result: i64 = stmt.query_row([], |row| row.get(0)).unwrap();
        assert_eq!(result, 1);

        // Drop handle to close channel and stop replicator
        drop(handle);
        let _ = replicator_task.await;
    }

    #[tokio::test]
    async fn test_sync_status() {
        let duckdb = Arc::new(DuckDbPool::in_memory().unwrap());
        let status = get_sync_status(&duckdb).await.unwrap();
        assert_eq!(status.latest_block, 0);
    }
}
