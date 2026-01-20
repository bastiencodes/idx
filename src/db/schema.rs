use anyhow::Result;
use tracing::info;

use super::Pool;

pub async fn run_migrations(pool: &Pool) -> Result<()> {
    let conn = pool.get().await?;

    info!("Running schema migrations");
    conn.batch_execute(include_str!("../../db/blocks.sql")).await?;
    conn.batch_execute(include_str!("../../db/txs.sql")).await?;
    conn.batch_execute(include_str!("../../db/logs.sql")).await?;
    conn.batch_execute(include_str!("../../db/receipts.sql")).await?;
    conn.batch_execute(include_str!("../../db/sync_state.sql")).await?;
    conn.batch_execute(include_str!("../../db/functions.sql")).await?;

    Ok(())
}


