use anyhow::Result;
use tracing::info;

use super::Pool;

/// Compress all uncompressed chunks except the most recent one.
/// Called automatically after backfill completes.
pub async fn compress_historical_chunks(pool: &Pool) -> Result<()> {
    info!("Starting compression of historical chunks");

    for table in &["blocks", "txs", "logs"] {
        compress_table(pool, table).await?;
    }

    info!("Compression complete");
    Ok(())
}

async fn compress_table(pool: &Pool, table: &str) -> Result<()> {
    let conn = pool.get().await?;

    // Check if this is a hypertable
    let is_hypertable: bool = conn
        .query_one(
            "SELECT EXISTS(SELECT 1 FROM timescaledb_information.hypertables WHERE hypertable_name = $1)",
            &[&table],
        )
        .await?
        .get(0);

    if !is_hypertable {
        info!(table = table, "Not a hypertable, skipping compression");
        return Ok(());
    }

    // Get all uncompressed chunks, ordered by range (oldest first)
    // Keep the most recent chunk uncompressed for writes
    let chunks: Vec<String> = conn
        .query(
            r#"
            SELECT chunk_schema || '.' || chunk_name as chunk_full_name
            FROM timescaledb_information.chunks
            WHERE hypertable_name = $1 
              AND is_compressed = false
            ORDER BY range_start ASC
            "#,
            &[&table],
        )
        .await?
        .iter()
        .map(|r| r.get::<_, String>(0))
        .collect();

    if chunks.len() <= 1 {
        info!(table = table, "No chunks to compress (keeping latest uncompressed)");
        return Ok(());
    }

    // Compress all but the last chunk
    let to_compress = &chunks[..chunks.len() - 1];
    info!(
        table = table,
        count = to_compress.len(),
        "Compressing chunks"
    );

    for chunk_name in to_compress {
        info!(table = table, chunk = chunk_name, "Compressing");
        conn.execute("SELECT compress_chunk($1::regclass)", &[&chunk_name])
            .await?;
    }

    // Log final stats
    let stats = conn
        .query_one(
            r#"
            SELECT 
                COUNT(*) FILTER (WHERE is_compressed) as compressed,
                COUNT(*) FILTER (WHERE NOT is_compressed) as uncompressed
            FROM timescaledb_information.chunks
            WHERE hypertable_name = $1
            "#,
            &[&table],
        )
        .await?;

    let compressed: i64 = stats.get(0);
    let uncompressed: i64 = stats.get(1);

    info!(
        table = table,
        compressed = compressed,
        uncompressed = uncompressed,
        "Compression stats"
    );

    Ok(())
}
