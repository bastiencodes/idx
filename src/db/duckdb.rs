use anyhow::{Context, Result};
use duckdb::Connection;
use tokio::sync::Mutex;

/// DuckDB connection pool for analytical queries.
///
/// Uses a single connection protected by mutex since DuckDB is single-writer
/// and in-memory databases can't be shared across connections.
pub struct DuckDbPool {
    /// Path to the DuckDB database file
    path: String,
    /// Single connection protected by mutex
    conn: Mutex<Connection>,
}

impl DuckDbPool {
    /// Creates a new DuckDB pool at the specified path.
    ///
    /// Creates the database file and schema if they don't exist.
    pub fn new(path: &str) -> Result<Self> {
        let conn = Connection::open(path).context("Failed to open DuckDB connection")?;

        // Run schema migrations
        run_schema(&conn)?;

        Ok(Self {
            path: path.to_string(),
            conn: Mutex::new(conn),
        })
    }

    /// Creates an in-memory DuckDB pool (useful for testing).
    pub fn in_memory() -> Result<Self> {
        Self::new(":memory:")
    }

    /// Gets the connection for queries or writes.
    pub async fn conn(&self) -> tokio::sync::MutexGuard<'_, Connection> {
        self.conn.lock().await
    }

    /// Returns the database path.
    pub fn path(&self) -> &str {
        &self.path
    }

    /// Gets the latest synced block number from DuckDB.
    pub async fn latest_block(&self) -> Result<Option<i64>> {
        let conn = self.conn().await;
        let mut stmt = conn.prepare("SELECT MAX(num) FROM blocks")?;
        let result: Option<i64> = stmt.query_row([], |row| row.get(0)).ok();
        Ok(result)
    }

    /// Executes a query and returns results as JSON.
    pub async fn query(&self, sql: &str) -> Result<(Vec<String>, Vec<Vec<serde_json::Value>>)> {
        let conn = self.conn().await;
        execute_query(&conn, sql)
    }
}

/// Creates the DuckDB schema matching PostgreSQL tables.
fn run_schema(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        r#"
        -- Blocks table (hex strings for hashes)
        CREATE TABLE IF NOT EXISTS blocks (
            num             BIGINT NOT NULL PRIMARY KEY,
            hash            VARCHAR NOT NULL,
            parent_hash     VARCHAR NOT NULL,
            timestamp       TIMESTAMPTZ NOT NULL,
            timestamp_ms    BIGINT NOT NULL,
            gas_limit       BIGINT NOT NULL,
            gas_used        BIGINT NOT NULL,
            miner           VARCHAR NOT NULL,
            extra_data      VARCHAR
        );

        -- Transactions table
        CREATE TABLE IF NOT EXISTS txs (
            block_num               BIGINT NOT NULL,
            block_timestamp         TIMESTAMPTZ NOT NULL,
            idx                     INTEGER NOT NULL,
            hash                    VARCHAR NOT NULL,
            type                    SMALLINT NOT NULL,
            "from"                  VARCHAR NOT NULL,
            "to"                    VARCHAR,
            value                   VARCHAR NOT NULL,
            input                   VARCHAR NOT NULL,
            gas_limit               BIGINT NOT NULL,
            max_fee_per_gas         VARCHAR NOT NULL,
            max_priority_fee_per_gas VARCHAR NOT NULL,
            gas_used                BIGINT,
            nonce_key               VARCHAR NOT NULL,
            nonce                   BIGINT NOT NULL,
            fee_token               VARCHAR,
            fee_payer               VARCHAR,
            calls                   JSON,
            call_count              SMALLINT NOT NULL DEFAULT 1,
            valid_before            BIGINT,
            valid_after             BIGINT,
            signature_type          SMALLINT,
            PRIMARY KEY (block_num, idx)
        );

        -- Logs table
        CREATE TABLE IF NOT EXISTS logs (
            block_num       BIGINT NOT NULL,
            block_timestamp TIMESTAMPTZ NOT NULL,
            log_idx         INTEGER NOT NULL,
            tx_idx          INTEGER NOT NULL,
            tx_hash         VARCHAR NOT NULL,
            address         VARCHAR NOT NULL,
            selector        VARCHAR,
            topics          VARCHAR[] NOT NULL,
            data            VARCHAR NOT NULL,
            PRIMARY KEY (block_num, log_idx)
        );

        -- Sync state tracking
        CREATE TABLE IF NOT EXISTS duckdb_sync_state (
            id              INTEGER PRIMARY KEY DEFAULT 1,
            latest_block    BIGINT NOT NULL DEFAULT 0,
            updated_at      TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
        );

        -- Initialize sync state if empty
        INSERT INTO duckdb_sync_state (id, latest_block)
        SELECT 1, 0
        WHERE NOT EXISTS (SELECT 1 FROM duckdb_sync_state WHERE id = 1);

        -- Create indexes for common query patterns
        CREATE INDEX IF NOT EXISTS idx_blocks_timestamp ON blocks (timestamp);
        CREATE INDEX IF NOT EXISTS idx_txs_hash ON txs (hash);
        CREATE INDEX IF NOT EXISTS idx_txs_from ON txs ("from");
        CREATE INDEX IF NOT EXISTS idx_txs_to ON txs ("to");
        CREATE INDEX IF NOT EXISTS idx_logs_address ON logs (address);
        CREATE INDEX IF NOT EXISTS idx_logs_selector ON logs (selector);
        CREATE INDEX IF NOT EXISTS idx_logs_tx_hash ON logs (tx_hash);

        -- ABI decoding macros (equivalent to PostgreSQL abi_* functions)
        -- DuckDB stores hex strings, so we work with substrings directly
        
        -- Extract uint256 from hex data at byte offset (returns HUGEINT for large values)
        CREATE OR REPLACE MACRO abi_uint(hex_data, byte_offset) AS (
            CASE 
                WHEN length(hex_data) >= 2 + (byte_offset + 32) * 2
                THEN ('0x' || substring(hex_data, 3 + byte_offset * 2, 64))::HUGEINT
                ELSE NULL
            END
        );

        -- Extract address from hex data at byte offset (last 20 bytes of 32-byte slot)
        CREATE OR REPLACE MACRO abi_address(hex_data, byte_offset) AS (
            CASE 
                WHEN length(hex_data) >= 2 + (byte_offset + 32) * 2
                THEN '0x' || substring(hex_data, 3 + byte_offset * 2 + 24, 40)
                ELSE NULL
            END
        );

        -- Extract bool from hex data at byte offset
        CREATE OR REPLACE MACRO abi_bool(hex_data, byte_offset) AS (
            CASE 
                WHEN length(hex_data) >= 2 + (byte_offset + 32) * 2
                THEN substring(hex_data, 3 + byte_offset * 2 + 62, 2) != '00'
                ELSE NULL
            END
        );

        -- Extract bytes32 from hex data at byte offset
        CREATE OR REPLACE MACRO abi_bytes32(hex_data, byte_offset) AS (
            CASE 
                WHEN length(hex_data) >= 2 + (byte_offset + 32) * 2
                THEN '0x' || substring(hex_data, 3 + byte_offset * 2, 64)
                ELSE NULL
            END
        );

        -- Extract topic (already a hex string in topics array)
        CREATE OR REPLACE MACRO topic_address(topic) AS (
            '0x' || substring(topic, 27, 40)
        );

        CREATE OR REPLACE MACRO topic_uint(topic) AS (
            topic::HUGEINT
        );
        "#,
    )
    .context("Failed to create DuckDB schema")?;

    Ok(())
}

/// Executes a query on DuckDB and returns results as JSON values.
pub fn execute_query(
    conn: &Connection,
    sql: &str,
) -> Result<(Vec<String>, Vec<Vec<serde_json::Value>>)> {
    let mut stmt = conn.prepare(sql).context("Failed to prepare DuckDB query")?;

    // Execute query and get rows iterator
    let mut rows_iter = stmt.query([]).context("Failed to execute DuckDB query")?;

    // Get column info from the statement after execution
    let column_count = rows_iter.as_ref().map(|r| r.column_count()).unwrap_or(0);
    let columns: Vec<String> = if let Some(first_row) = rows_iter.as_ref() {
        (0..column_count)
            .map(|i| {
                first_row
                    .column_name(i)
                    .map(|s| s.to_string())
                    .unwrap_or_else(|_| "?".to_string())
            })
            .collect()
    } else {
        vec![]
    };

    // Collect all rows
    let mut result_rows = Vec::new();
    while let Some(row) = rows_iter.next()? {
        let mut values = Vec::with_capacity(column_count);
        for i in 0..column_count {
            let value = row_to_json_value(&row, i);
            values.push(value);
        }
        result_rows.push(values);
    }

    Ok((columns, result_rows))
}

/// Converts a DuckDB row column to a JSON value.
fn row_to_json_value(row: &duckdb::Row<'_>, idx: usize) -> serde_json::Value {
    // Try different types in order of likelihood
    if let Ok(v) = row.get::<_, i64>(idx) {
        return serde_json::Value::Number(v.into());
    }
    if let Ok(v) = row.get::<_, f64>(idx) {
        return serde_json::Number::from_f64(v)
            .map(serde_json::Value::Number)
            .unwrap_or(serde_json::Value::Null);
    }
    if let Ok(v) = row.get::<_, String>(idx) {
        return serde_json::Value::String(v);
    }
    if let Ok(v) = row.get::<_, bool>(idx) {
        return serde_json::Value::Bool(v);
    }
    if let Ok(v) = row.get::<_, i32>(idx) {
        return serde_json::Value::Number(v.into());
    }
    if let Ok(v) = row.get::<_, Option<String>>(idx) {
        return v
            .map(serde_json::Value::String)
            .unwrap_or(serde_json::Value::Null);
    }

    // Fallback to null
    serde_json::Value::Null
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_in_memory_pool() {
        let pool = DuckDbPool::in_memory().unwrap();
        assert_eq!(pool.path(), ":memory:");
    }

    #[tokio::test]
    async fn test_schema_creation() {
        let pool = DuckDbPool::in_memory().unwrap();
        let conn = pool.conn().await;

        // Verify tables exist
        let (_, rows) = execute_query(
            &conn,
            "SELECT table_name FROM information_schema.tables WHERE table_schema = 'main'",
        )
        .unwrap();

        let table_names: Vec<&str> = rows
            .iter()
            .filter_map(|r| r.first().and_then(|v| v.as_str()))
            .collect();

        assert!(table_names.contains(&"blocks"));
        assert!(table_names.contains(&"txs"));
        assert!(table_names.contains(&"logs"));
        assert!(table_names.contains(&"duckdb_sync_state"));
    }

    #[tokio::test]
    async fn test_insert_and_query() {
        let pool = DuckDbPool::in_memory().unwrap();

        // Insert a block and query it back
        let conn = pool.conn().await;
        conn.execute(
            "INSERT INTO blocks (num, hash, parent_hash, timestamp, timestamp_ms, gas_limit, gas_used, miner)
             VALUES (1, '0xabc', '0x000', '2024-01-01 00:00:00+00', 1704067200000, 30000000, 21000, '0xminer')",
            [],
        )
        .unwrap();

        let (_, rows) = execute_query(&conn, "SELECT num, hash FROM blocks WHERE num = 1").unwrap();

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0][0], serde_json::json!(1));
        assert_eq!(rows[0][1], serde_json::json!("0xabc"));
    }

    #[tokio::test]
    async fn test_aggregation_query() {
        let pool = DuckDbPool::in_memory().unwrap();

        // Insert multiple blocks
        let conn = pool.conn().await;
        for i in 1..=10 {
            conn.execute(
                &format!(
                    "INSERT INTO blocks (num, hash, parent_hash, timestamp, timestamp_ms, gas_limit, gas_used, miner)
                     VALUES ({i}, '0x{i:03x}', '0x000', '2024-01-01 00:00:00+00', {}, 30000000, {}, '0xminer')",
                    1_704_067_200_000_i64 + i * 1000,
                    21000 * i
                ),
                [],
            )
            .unwrap();
        }

        // Run aggregation
        let (cols, rows) =
            execute_query(&conn, "SELECT COUNT(*) as cnt, SUM(gas_used) as total FROM blocks")
                .unwrap();

        assert_eq!(cols, vec!["cnt", "total"]);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0][0], serde_json::json!(10));
        // Sum of 21000 * (1+2+...+10) = 21000 * 55 = 1155000
        assert_eq!(rows[0][1], serde_json::json!(1155000));
    }
}
