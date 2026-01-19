use anyhow::Result;
use clap::Args as ClapArgs;
use metrics_exporter_prometheus::PrometheusBuilder;
use std::net::SocketAddr;
use tracing::info;

use ak47::api;
use ak47::config::Config;
use ak47::db;
use ak47::sync::engine::SyncEngine;

#[derive(ClapArgs)]
pub struct Args {
    /// Chain name (presto, andantino, moderato) - uses preset RPC URL
    #[arg(long, env = "AK47_CHAIN")]
    pub chain: Option<String>,

    /// RPC endpoint URL (overrides --chain)
    #[arg(long, env = "AK47_RPC_URL")]
    pub rpc: Option<String>,

    /// Database URL
    #[arg(long, env = "AK47_DATABASE_URL")]
    pub db: String,

    /// HTTP API port (0 to disable)
    #[arg(long, default_value = "8080")]
    pub port: u16,

    /// HTTP API bind address
    #[arg(long, default_value = "0.0.0.0")]
    pub bind: String,

    /// Metrics port (0 to disable)
    #[arg(long, default_value = "9090")]
    pub metrics_port: u16,
}

pub async fn run(args: Args) -> Result<()> {
    let rpc_url = match (&args.rpc, &args.chain) {
        (Some(rpc), _) => rpc.clone(),
        (None, Some(chain_name)) => {
            let chain = ak47::config::get_chain(chain_name)
                .ok_or_else(|| anyhow::anyhow!(
                    "Unknown chain '{}'. Available: presto, andantino, moderato",
                    chain_name
                ))?;
            info!(chain = chain.name, rpc = chain.rpc_url, "Using preset chain config");
            chain.rpc_url.to_string()
        }
        (None, None) => {
            return Err(anyhow::anyhow!(
                "Either --chain or --rpc must be specified"
            ));
        }
    };

    let config = Config {
        rpc_url,
        database_url: args.db,
    };

    if args.metrics_port > 0 {
        let metrics_addr: SocketAddr = format!("{}:{}", args.bind, args.metrics_port).parse()?;
        info!(addr = %metrics_addr, "Starting Prometheus metrics server");
        PrometheusBuilder::new()
            .with_http_listener(metrics_addr)
            .install()?;
    }

    info!("Connecting to database...");
    let pool = db::create_pool(&config.database_url).await?;

    info!("Running migrations...");
    db::run_migrations(&pool).await?;

    let (shutdown_tx, shutdown_rx) = tokio::sync::broadcast::channel(1);

    tokio::spawn({
        let shutdown_tx = shutdown_tx.clone();
        async move {
            tokio::signal::ctrl_c().await.ok();
            info!("Shutting down...");
            let _ = shutdown_tx.send(());
        }
    });

    if args.port > 0 {
        let addr: SocketAddr = format!("{}:{}", args.bind, args.port).parse()?;
        let router = api::router(pool.clone());

        info!(addr = %addr, "Starting HTTP API server");

        let listener = tokio::net::TcpListener::bind(addr).await?;
        let mut shutdown_rx_api = shutdown_tx.subscribe();

        tokio::spawn(async move {
            axum::serve(listener, router)
                .with_graceful_shutdown(async move {
                    let _ = shutdown_rx_api.recv().await;
                })
                .await
                .ok();
        });
    }

    info!("Starting sync engine...");
    let mut engine = SyncEngine::new(pool, &config.rpc_url).await?;

    engine.run(shutdown_rx).await
}
