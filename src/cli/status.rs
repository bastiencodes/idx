use anyhow::Result;
use clap::Args as ClapArgs;
use std::path::PathBuf;

use ak47::config::Config;
use ak47::db;
use ak47::sync::fetcher::RpcClient;
use ak47::sync::writer::load_all_sync_states;

#[derive(ClapArgs)]
pub struct Args {
    /// Path to config file
    #[arg(short, long, default_value = "config.toml")]
    pub config: PathBuf,

    /// Watch mode - continuously update status
    #[arg(long, short)]
    pub watch: bool,

    /// Output as JSON
    #[arg(long)]
    pub json: bool,
}

pub async fn run(args: Args) -> Result<()> {
    let config = Config::load(&args.config)?;
    let pool = db::create_pool(&config.database_url).await?;

    loop {
        if args.watch {
            print!("\x1B[2J\x1B[1;1H");
        }

        if args.json {
            print_json_status(&config, &pool).await?;
        } else {
            print_status(&config, &pool).await?;
        }

        if !args.watch {
            break;
        }

        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    }

    Ok(())
}

async fn print_status(config: &Config, pool: &db::Pool) -> Result<()> {
    println!("╔═══════════════════════════════════════════════════════════╗");
    println!("║                     AK47 Indexer Status                   ║");
    println!("╚═══════════════════════════════════════════════════════════╝");
    println!();

    let states = load_all_sync_states(pool).await?;

    for chain in &config.chains {
        let rpc = RpcClient::new(&chain.rpc_url);
        let live_head = rpc.latest_block_number().await.ok();

        println!("┌─ {} (chain_id: {}) ─────────────────────", chain.name, chain.chain_id);
        println!("│");

        if let Some(state) = states.iter().find(|s| s.chain_id == chain.chain_id) {
            let head = live_head.unwrap_or(state.head_num);
            let lag = head.saturating_sub(state.synced_num);

            // Forward sync status
            println!("│  Forward Sync");
            println!("│  ├─ Head:      {} {}", head, if live_head.is_some() { "(live)" } else { "" });
            println!("│  ├─ Synced:    {}", state.synced_num);
            println!("│  └─ Lag:       {lag} blocks");
            println!("│");

            // Backfill status
            let remaining = state.backfill_remaining();
            println!("│  Backfill");
            match state.backfill_num {
                None if remaining > 0 => {
                    println!("│  ├─ Status:   Pending");
                    println!("│  └─ Needed:   {} blocks (0 → {})", format_number(remaining), state.synced_num.saturating_sub(1));
                }
                None => {
                    println!("│  └─ Status:   Not needed");
                }
                Some(0) => {
                    println!("│  └─ Status:   ✓ Complete (genesis reached)");
                }
                Some(n) => {
                    let total = state.synced_num;
                    let done = state.synced_num.saturating_sub(n);
                    let pct = if total > 0 {
                        (done as f64 / total as f64 * 100.0) as u64
                    } else {
                        0
                    };
                    println!("│  ├─ Status:   In progress");
                    println!("│  ├─ Position: block {}", format_number(n));
                    println!("│  ├─ Remaining: {} blocks", format_number(n));
                    println!("│  ├─ Progress: {pct}%");
                    if let Some(rate) = state.sync_rate() {
                        println!("│  ├─ Rate:     {:.0} blk/s", rate);
                    }
                    if let Some(eta) = state.backfill_eta_secs() {
                        println!("│  └─ ETA:      {}", format_eta(eta));
                    } else {
                        println!("│  └─ ETA:      calculating...");
                    }
                }
            }

            // Coverage
            let (low, high) = state.indexed_range();
            println!("│");
            println!("│  Coverage");
            println!("│  ├─ Range:    {} → {}", format_number(low), format_number(high));
            println!("│  └─ Total:    {} blocks", format_number(state.total_indexed()));
        } else {
            println!("│  Status: Not syncing");
            if let Some(head) = live_head {
                println!("│  Head: {head} (live)");
            }
        }

        println!("└───────────────────────────────────────────────────────────");
        println!();
    }

    Ok(())
}

async fn print_json_status(config: &Config, pool: &db::Pool) -> Result<()> {
    let states = load_all_sync_states(pool).await?;
    let mut chains = Vec::new();

    for chain in &config.chains {
        let rpc = RpcClient::new(&chain.rpc_url);
        let live_head = rpc.latest_block_number().await.ok();
        let state = states.iter().find(|s| s.chain_id == chain.chain_id);

        let chain_status = serde_json::json!({
            "name": chain.name,
            "chain_id": chain.chain_id,
            "rpc_url": chain.rpc_url,
            "head": live_head,
            "synced": state.map(|s| s.synced_num),
            "lag": state.and_then(|s| live_head.map(|h| h.saturating_sub(s.synced_num))),
            "backfill_block": state.and_then(|s| s.backfill_num),
            "backfill_complete": state.map(|s| s.backfill_complete()).unwrap_or(false),
            "sync_rate": state.and_then(|s| s.sync_rate()),
            "backfill_eta_secs": state.and_then(|s| s.backfill_eta_secs()),
        });
        chains.push(chain_status);
    }

    let output = serde_json::json!({ "chains": chains });
    println!("{}", serde_json::to_string_pretty(&output)?);
    Ok(())
}

fn format_number(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}K", n as f64 / 1_000.0)
    } else {
        n.to_string()
    }
}

fn format_eta(secs: f64) -> String {
    if secs <= 0.0 || secs.is_nan() || secs.is_infinite() {
        return "unknown".to_string();
    }

    let secs = secs as u64;
    if secs < 60 {
        format!("{secs}s")
    } else if secs < 3600 {
        format!("{}m {}s", secs / 60, secs % 60)
    } else if secs < 86400 {
        format!("{}h {}m", secs / 3600, (secs % 3600) / 60)
    } else {
        format!("{}d {}h", secs / 86400, (secs % 86400) / 3600)
    }
}
