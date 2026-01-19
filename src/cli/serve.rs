use anyhow::Result;
use clap::Args as ClapArgs;
use std::net::SocketAddr;
use tracing::info;

use ak47::api;
use ak47::db;

#[derive(ClapArgs)]
pub struct Args {
    /// Database URL
    #[arg(long, env = "AK47_DATABASE_URL")]
    pub db: String,

    /// HTTP API port
    #[arg(long, default_value = "8080")]
    pub port: u16,

    /// HTTP API bind address
    #[arg(long, default_value = "0.0.0.0")]
    pub bind: String,
}

pub async fn run(args: Args) -> Result<()> {
    info!("Connecting to database...");
    let pool = db::create_pool(&args.db).await?;

    let addr: SocketAddr = format!("{}:{}", args.bind, args.port).parse()?;
    let router = api::router(pool);

    info!(addr = %addr, "Starting HTTP API server");

    let listener = tokio::net::TcpListener::bind(addr).await?;

    let (shutdown_tx, mut shutdown_rx) = tokio::sync::broadcast::channel::<()>(1);

    tokio::spawn(async move {
        tokio::signal::ctrl_c().await.ok();
        info!("Shutting down...");
        let _ = shutdown_tx.send(());
    });

    axum::serve(listener, router)
        .with_graceful_shutdown(async move {
            let _ = shutdown_rx.recv().await;
        })
        .await?;

    Ok(())
}
