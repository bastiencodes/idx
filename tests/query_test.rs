mod common;

use common::testdb::TestDb;
use ak47::query::EventSignature;

#[tokio::test]
async fn test_abi_uint_function() {
    let db = TestDb::empty().await;
    let conn = db.pool.get().await.expect("Failed to get connection");

    // Test abi_uint with a known value: 0x...01 = 1
    let result: String = conn
        .query_one(
            "SELECT abi_uint('\\x0000000000000000000000000000000000000000000000000000000000000001'::bytea)::text",
            &[],
        )
        .await
        .expect("Failed to execute abi_uint")
        .get(0);

    assert_eq!(result, "1");

    // Test with larger value: 256
    let result: String = conn
        .query_one(
            "SELECT abi_uint('\\x0000000000000000000000000000000000000000000000000000000000000100'::bytea)::text",
            &[],
        )
        .await
        .expect("Failed to execute abi_uint")
        .get(0);

    assert_eq!(result, "256");
}

#[tokio::test]
async fn test_abi_address_function() {
    let db = TestDb::empty().await;
    let conn = db.pool.get().await.expect("Failed to get connection");

    // Test abi_address: extracts last 20 bytes from 32-byte input
    // Input: 0x000000000000000000000000deadbeefdeadbeefdeadbeefdeadbeefdeadbeef
    let result: Vec<u8> = conn
        .query_one(
            "SELECT abi_address('\\x000000000000000000000000deadbeefdeadbeefdeadbeefdeadbeefdeadbeef'::bytea)",
            &[],
        )
        .await
        .expect("Failed to execute abi_address")
        .get(0);

    assert_eq!(hex::encode(&result), "deadbeefdeadbeefdeadbeefdeadbeefdeadbeef");
}

#[tokio::test]
async fn test_abi_bool_function() {
    let db = TestDb::empty().await;
    let conn = db.pool.get().await.expect("Failed to get connection");

    // Test abi_bool: true
    let result: bool = conn
        .query_one(
            "SELECT abi_bool('\\x0000000000000000000000000000000000000000000000000000000000000001'::bytea)",
            &[],
        )
        .await
        .expect("Failed to execute abi_bool")
        .get(0);

    assert!(result);

    // Test abi_bool: false
    let result: bool = conn
        .query_one(
            "SELECT abi_bool('\\x0000000000000000000000000000000000000000000000000000000000000000'::bytea)",
            &[],
        )
        .await
        .expect("Failed to execute abi_bool")
        .get(0);

    assert!(!result);
}

#[tokio::test]
async fn test_event_signature_selector() {
    // Test Transfer selector matches known value
    let sig = EventSignature::parse("Transfer(address,address,uint256)").unwrap();
    assert_eq!(sig.selector_hex(), "ddf252ad");

    // Test Approval selector
    let sig = EventSignature::parse("Approval(address,address,uint256)").unwrap();
    assert_eq!(sig.selector_hex(), "8c5be1e5");
}

#[tokio::test]
async fn test_cte_query_syntax() {
    let db = TestDb::empty().await;
    let conn = db.pool.get().await.expect("Failed to get connection");

    let sig = EventSignature::parse("Transfer(address indexed from, address indexed to, uint256 value)").unwrap();
    let cte = sig.to_cte_sql();

    // Build a full query and verify it's valid SQL (should return 0 rows on empty DB)
    // Note: CTE uses lowercase name, so we reference it as 'transfer' not 'Transfer'
    let sql = format!(
        r#"WITH {cte}
SELECT * FROM transfer
WHERE block_timestamp > NOW() - INTERVAL '1 hour'
LIMIT 10"#
    );

    let rows = conn
        .query(&sql, &[])
        .await
        .expect("CTE query should be valid SQL");

    // On an empty/truncated DB, should return 0 rows but execute successfully
    assert!(rows.len() <= 10);
}

#[tokio::test]
async fn test_format_address_function() {
    let db = TestDb::empty().await;
    let conn = db.pool.get().await.expect("Failed to get connection");

    let result: String = conn
        .query_one(
            "SELECT format_address('\\xdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef'::bytea)",
            &[],
        )
        .await
        .expect("Failed to execute format_address")
        .get(0);

    assert_eq!(result, "0xdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef");
}

#[tokio::test]
async fn test_abi_int_function() {
    let db = TestDb::empty().await;
    let conn = db.pool.get().await.expect("Failed to get connection");

    // Test positive int: 1
    let result: String = conn
        .query_one(
            "SELECT abi_int('\\x0000000000000000000000000000000000000000000000000000000000000001'::bytea)::text",
            &[],
        )
        .await
        .expect("Failed to execute abi_int")
        .get(0);

    assert_eq!(result, "1");

    // Test negative int: -1 (all 0xFF bytes in two's complement)
    let result: String = conn
        .query_one(
            "SELECT abi_int('\\xffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff'::bytea)::text",
            &[],
        )
        .await
        .expect("Failed to execute abi_int")
        .get(0);

    assert_eq!(result, "-1");
}

// DuckDB tests
use ak47::db::DuckDbPool;

#[tokio::test]
async fn test_duckdb_basic_queries() {
    let pool = DuckDbPool::new(":memory:").expect("Failed to create DuckDB pool");
    let conn = pool.conn().await;

    // Insert test data
    conn.execute_batch(
        r#"
        INSERT INTO blocks (num, hash, parent_hash, timestamp, timestamp_ms, gas_limit, gas_used, miner, extra_data)
        VALUES 
            (1, '0x1111', '0x0000', '2024-01-01 00:00:00', 1704067200000, 30000000, 21000, '0xminer', NULL),
            (2, '0x2222', '0x1111', '2024-01-01 00:01:00', 1704067260000, 30000000, 42000, '0xminer', NULL),
            (3, '0x3333', '0x2222', '2024-01-01 00:02:00', 1704067320000, 30000000, 63000, '0xminer', NULL);
        "#,
    ).expect("Failed to insert test blocks");

    // Test COUNT
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM blocks", [], |row| row.get(0))
        .expect("COUNT query failed");
    assert_eq!(count, 3);

    // Test SUM
    let sum: i64 = conn
        .query_row("SELECT SUM(gas_used) FROM blocks", [], |row| row.get(0))
        .expect("SUM query failed");
    assert_eq!(sum, 126000);

    // Test AVG
    let avg: f64 = conn
        .query_row("SELECT AVG(gas_used) FROM blocks", [], |row| row.get(0))
        .expect("AVG query failed");
    assert!((avg - 42000.0).abs() < 0.01);
}

#[tokio::test]
async fn test_duckdb_group_by() {
    let pool = DuckDbPool::new(":memory:").expect("Failed to create DuckDB pool");
    let conn = pool.conn().await;

    conn.execute_batch(
        r#"
        INSERT INTO blocks (num, hash, parent_hash, timestamp, timestamp_ms, gas_limit, gas_used, miner, extra_data)
        VALUES 
            (1, '0x1', '0x0', '2024-01-01 00:00:00', 1704067200000, 30000000, 21000, '0xminer1', NULL),
            (2, '0x2', '0x1', '2024-01-01 00:01:00', 1704067260000, 30000000, 21000, '0xminer1', NULL),
            (3, '0x3', '0x2', '2024-01-01 00:02:00', 1704067320000, 30000000, 42000, '0xminer2', NULL);
        "#,
    ).expect("Failed to insert test blocks");

    // GROUP BY gas_used
    let mut stmt = conn
        .prepare("SELECT gas_used, COUNT(*) as cnt FROM blocks GROUP BY gas_used ORDER BY cnt DESC")
        .expect("Failed to prepare query");

    let results: Vec<(i64, i64)> = stmt
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .expect("Query failed")
        .collect::<Result<Vec<_>, _>>()
        .expect("Failed to collect results");

    assert_eq!(results.len(), 2);
    assert_eq!(results[0], (21000, 2)); // 21000 appears twice
    assert_eq!(results[1], (42000, 1)); // 42000 appears once
}

#[tokio::test]
async fn test_duckdb_window_function() {
    let pool = DuckDbPool::new(":memory:").expect("Failed to create DuckDB pool");
    let conn = pool.conn().await;

    conn.execute_batch(
        r#"
        INSERT INTO blocks (num, hash, parent_hash, timestamp, timestamp_ms, gas_limit, gas_used, miner, extra_data)
        VALUES 
            (1, '0x1', '0x0', '2024-01-01 00:00:00', 1704067200000, 30000000, 100, '0xminer', NULL),
            (2, '0x2', '0x1', '2024-01-01 00:01:00', 1704067260000, 30000000, 200, '0xminer', NULL),
            (3, '0x3', '0x2', '2024-01-01 00:02:00', 1704067320000, 30000000, 300, '0xminer', NULL);
        "#,
    ).expect("Failed to insert test blocks");

    // ROW_NUMBER window function
    let mut stmt = conn
        .prepare("SELECT num, gas_used, ROW_NUMBER() OVER (ORDER BY num) as rn FROM blocks")
        .expect("Failed to prepare query");

    let results: Vec<(i64, i64, i64)> = stmt
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
        .expect("Query failed")
        .collect::<Result<Vec<_>, _>>()
        .expect("Failed to collect results");

    assert_eq!(results.len(), 3);
    assert_eq!(results[0], (1, 100, 1));
    assert_eq!(results[1], (2, 200, 2));
    assert_eq!(results[2], (3, 300, 3));
}

#[tokio::test]
async fn test_duckdb_txs_and_logs() {
    let pool = DuckDbPool::new(":memory:").expect("Failed to create DuckDB pool");
    let conn = pool.conn().await;

    // Insert txs
    conn.execute_batch(
        r#"
        INSERT INTO txs (block_num, block_timestamp, idx, hash, type, "from", "to", value, input, gas_limit, max_fee_per_gas, max_priority_fee_per_gas, gas_used, nonce_key, nonce, call_count)
        VALUES 
            (1, '2024-01-01 00:00:00', 0, '0xtx1', 2, '0xfrom1', '0xto1', '1000000', '0x', 21000, '1000000000', '1000000', 21000, '0xkey', 0, 1),
            (1, '2024-01-01 00:00:00', 1, '0xtx2', 2, '0xfrom2', '0xto2', '2000000', '0x', 21000, '1000000000', '1000000', 21000, '0xkey', 1, 1);
        "#,
    ).expect("Failed to insert test txs");

    // Insert logs
    conn.execute_batch(
        r#"
        INSERT INTO logs (block_num, block_timestamp, log_idx, tx_idx, tx_hash, address, selector, topics, data)
        VALUES 
            (1, '2024-01-01 00:00:00', 0, 0, '0xtx1', '0xcontract', '0xddf252ad', ['0xtopic1'], '0xdata');
        "#,
    ).expect("Failed to insert test logs");

    // Query txs
    let tx_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM txs", [], |row| row.get(0))
        .expect("Txs count failed");
    assert_eq!(tx_count, 2);

    // Query logs
    let log_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM logs", [], |row| row.get(0))
        .expect("Logs count failed");
    assert_eq!(log_count, 1);

    // Aggregation across txs
    let total_gas: i64 = conn
        .query_row("SELECT SUM(gas_used) FROM txs", [], |row| row.get(0))
        .expect("SUM gas_used failed");
    assert_eq!(total_gas, 42000);
}

#[tokio::test]
async fn test_duckdb_abi_macros() {
    let pool = DuckDbPool::new(":memory:").expect("Failed to create DuckDB pool");
    let conn = pool.conn().await;

    // Test abi_address macro - extracts last 20 bytes from 32-byte slot
    let result: String = conn
        .query_row(
            "SELECT abi_address('0x000000000000000000000000deadbeefdeadbeefdeadbeefdeadbeefdeadbeef', 0)",
            [],
            |row| row.get(0),
        )
        .expect("abi_address failed");
    assert_eq!(result, "0xdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef");

    // Test abi_bool macro
    let result: bool = conn
        .query_row(
            "SELECT abi_bool('0x0000000000000000000000000000000000000000000000000000000000000001', 0)",
            [],
            |row| row.get(0),
        )
        .expect("abi_bool failed");
    assert!(result);

    // Test abi_bytes32 macro
    let result: String = conn
        .query_row(
            "SELECT abi_bytes32('0xdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef', 0)",
            [],
            |row| row.get(0),
        )
        .expect("abi_bytes32 failed");
    assert_eq!(result, "0xdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef");
}
