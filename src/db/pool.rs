use anyhow::Result;
use deadpool_postgres::{Config, Pool, Runtime};
use tokio_postgres::NoTls;

pub async fn create_pool(database_url: &str) -> Result<Pool> {
    create_pool_with_size(database_url, 16).await
}

pub async fn create_pool_with_size(database_url: &str, max_size: usize) -> Result<Pool> {
    let mut config = Config::new();
    config.url = Some(database_url.to_string());
    config.pool = Some(deadpool_postgres::PoolConfig {
        max_size,
        ..Default::default()
    });

    let pool = config.create_pool(Some(Runtime::Tokio1), NoTls)?;

    let _ = pool.get().await?;

    Ok(pool)
}
