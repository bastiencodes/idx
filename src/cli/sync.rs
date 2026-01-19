use anyhow::Result;
use clap::{Args as ClapArgs, Subcommand};
use tokio::sync::broadcast;
use tracing::info;

use ak47::config::chain_name;
use ak47::db::{self, PartitionManager};
use ak47::sync::engine::SyncEngine;
use ak47::sync::fetcher::RpcClient;
use ak47::sync::writer::load_sync_state;

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

    /// Skip running migrations
    #[arg(long)]
    pub skip_migrations: bool,

    #[command(subcommand)]
    pub command: SyncCommands,
}

#[derive(Subcommand)]
pub enum SyncCommands {
    /// Backfill blocks going backwards from head toward genesis
    Backfill {
        /// Start block number (defaults to current synced_num or chain head)
        #[arg(long)]
        from: Option<u64>,

        /// Target block number to backfill to (default: 0 = genesis)
        #[arg(long, default_value = "0")]
        to: u64,

        /// Batch size for RPC requests
        #[arg(long, default_value = "100")]
        batch_size: u64,
    },

    /// Show sync status
    Status,
}

pub async fn run(args: Args) -> Result<()> {
    let rpc_url = resolve_rpc(&args.rpc, &args.chain)?;

    let pool = db::create_pool(&args.db).await?;
    if !args.skip_migrations {
        db::run_migrations(&pool).await?;
    }

    match args.command {
        SyncCommands::Backfill {
            from,
            to,
            batch_size,
        } => {
            run_backfill(&pool, &rpc_url, from, to, batch_size).await?;
        }
        SyncCommands::Status => {
            run_status(&pool, &rpc_url).await?;
        }
    }

    Ok(())
}

fn resolve_rpc(rpc: &Option<String>, chain: &Option<String>) -> Result<String> {
    match (rpc, chain) {
        (Some(rpc), _) => Ok(rpc.clone()),
        (None, Some(chain_name)) => {
            let chain = ak47::config::get_chain(chain_name).ok_or_else(|| {
                anyhow::anyhow!(
                    "Unknown chain '{}'. Available: presto, andantino, moderato",
                    chain_name
                )
            })?;
            info!(
                chain = chain.name,
                rpc = chain.rpc_url,
                "Using preset chain config"
            );
            Ok(chain.rpc_url.to_string())
        }
        (None, None) => Err(anyhow::anyhow!("Either --chain or --rpc must be specified")),
    }
}

async fn run_backfill(
    pool: &db::Pool,
    rpc_url: &str,
    from: Option<u64>,
    to: u64,
    batch_size: u64,
) -> Result<()> {
    let engine = SyncEngine::new(pool.clone(), rpc_url).await?;
    let partitions = PartitionManager::new(pool.clone());

    // Determine starting block
    let state = engine.status().await?;
    let start_block = match from {
        Some(n) => n,
        None => {
            // Default to synced_num if we have state, otherwise get chain head
            if state.synced_num > 0 {
                state.synced_num
            } else {
                let rpc = RpcClient::new(rpc_url);
                rpc.latest_block_number().await?
            }
        }
    };

    // Ensure partitions exist for the range
    partitions.ensure_partition(start_block).await?;
    if to > 0 {
        partitions.ensure_partition(to).await?;
    }

    let (shutdown_tx, shutdown_rx) = broadcast::channel(1);

    // Set up Ctrl+C handler
    let shutdown_tx_clone = shutdown_tx.clone();
    tokio::spawn(async move {
        tokio::signal::ctrl_c().await.ok();
        info!("Received Ctrl+C, shutting down gracefully...");
        let _ = shutdown_tx_clone.send(());
    });

    let synced = engine.backfill(start_block, to, batch_size, shutdown_rx).await?;

    info!(
        synced = synced,
        from = start_block,
        to = to,
        "Backfill complete"
    );

    Ok(())
}

async fn run_status(pool: &db::Pool, rpc_url: &str) -> Result<()> {
    let rpc = RpcClient::new(rpc_url);
    let chain_id = rpc.chain_id().await?;
    let remote_head = rpc.latest_block_number().await?;

    let state = load_sync_state(pool).await?.unwrap_or_default();

    let (low, high) = state.indexed_range();
    let indexed_blocks = if high >= low { high - low + 1 } else { 0 };

    println!("=== AK47 Sync Status ===");
    println!();
    println!("Chain:          {} ({})", chain_name(chain_id), chain_id);
    println!("Remote head:    {}", remote_head);
    println!();
    println!("Forward sync:");
    println!("  Synced to:    {}", state.synced_num);
    println!("  Head lag:     {} blocks", remote_head.saturating_sub(state.synced_num));
    println!();
    println!("Backfill:");
    match state.backfill_num {
        None => println!("  Status:       Not started"),
        Some(0) => println!("  Status:       Complete (reached genesis)"),
        Some(n) => {
            println!("  Status:       In progress");
            println!("  Backfilled to: {}", n);
            println!("  Remaining:    {} blocks to genesis", n);
        }
    }
    println!();
    println!("Coverage:");
    println!("  Indexed range: {} - {}", low, high);
    println!("  Total blocks:  {}", indexed_blocks);

    // Check for gaps
    let gaps = ak47::sync::writer::detect_gaps(pool).await?;
    if gaps.is_empty() {
        println!("  Gaps:         None");
    } else {
        println!("  Gaps:         {} gap(s) detected", gaps.len());
        for (start, end) in &gaps {
            println!("    - {} to {} ({} blocks)", start, end, end - start + 1);
        }
    }

    Ok(())
}
