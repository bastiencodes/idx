use anyhow::{anyhow, Result};
use clap::{Args as ClapArgs, Subcommand};
use std::path::PathBuf;

use ak47::config::Config;
use ak47::db;

#[derive(ClapArgs)]
pub struct Args {
    /// Path to config file
    #[arg(short, long, default_value = "config.toml")]
    pub config: PathBuf,

    /// Chain name (uses first chain if not specified)
    #[arg(long)]
    pub chain: Option<String>,

    #[command(subcommand)]
    pub command: MaterializeCommands,
}

#[derive(Subcommand)]
pub enum MaterializeCommands {
    /// Create a new continuous aggregate
    Create {
        /// Name of the materialized view
        name: String,

        /// SQL SELECT query (must use time_bucket and GROUP BY)
        /// Example: "SELECT time_bucket('1 hour', block_timestamp) AS hour, COUNT(*) AS tx_count FROM txs GROUP BY hour"
        #[arg(long)]
        sql: String,

        /// Refresh interval (e.g., "5 minutes", "1 hour")
        #[arg(long, default_value = "5 minutes")]
        refresh_interval: String,

        /// How far back to refresh on each run (e.g., "2 hours")
        #[arg(long, default_value = "2 hours")]
        refresh_start: String,

        /// Skip backfilling historical data after creation
        #[arg(long)]
        no_backfill: bool,
    },

    /// List all continuous aggregates
    List,

    /// Drop a continuous aggregate
    Drop {
        /// Name of the materialized view to drop
        name: String,
    },

    /// Manually refresh a continuous aggregate
    Refresh {
        /// Name of the materialized view
        name: String,

        /// Refresh all historical data (NULL, NULL)
        #[arg(long)]
        full: bool,
    },

    /// Show details about a continuous aggregate
    Info {
        /// Name of the materialized view
        name: String,
    },
}

pub async fn run(args: Args) -> Result<()> {
    let config = Config::load(&args.config)?;

    let chain = if let Some(name) = &args.chain {
        config
            .chains
            .iter()
            .find(|c| c.name.eq_ignore_ascii_case(name))
            .ok_or_else(|| anyhow!("Chain '{}' not found in config", name))?
    } else {
        config
            .chains
            .first()
            .ok_or_else(|| anyhow!("No chains configured"))?
    };

    let pool = db::create_pool(&chain.database_url).await?;
    let conn = pool.get().await?;

    match args.command {
        MaterializeCommands::Create {
            name,
            sql,
            refresh_interval,
            refresh_start,
            no_backfill,
        } => {
            let sql_lower = sql.to_lowercase();
            if !sql_lower.contains("time_bucket") {
                return Err(anyhow!("SQL must use time_bucket() for continuous aggregates"));
            }
            if !sql_lower.contains("group by") {
                return Err(anyhow!("SQL must have a GROUP BY clause"));
            }

            let create_sql = format!(
                r#"
                CREATE MATERIALIZED VIEW IF NOT EXISTS {name}
                WITH (timescaledb.continuous) AS
                {sql}
                WITH NO DATA
                "#,
            );

            println!("Creating continuous aggregate '{}'...", name);
            conn.execute(&create_sql, &[]).await?;

            let policy_sql = format!(
                r#"
                SELECT add_continuous_aggregate_policy('{name}',
                    start_offset => INTERVAL '{refresh_start}',
                    end_offset => INTERVAL '1 minute',
                    schedule_interval => INTERVAL '{refresh_interval}',
                    if_not_exists => true
                )
                "#,
            );
            conn.execute(&policy_sql, &[]).await?;

            println!("✓ Created with refresh every {}", refresh_interval);

            if !no_backfill {
                println!("Backfilling historical data...");
                let refresh_sql = format!(
                    "CALL refresh_continuous_aggregate('{}', NULL, NULL)",
                    name
                );
                conn.execute(&refresh_sql, &[]).await?;
                println!("✓ Backfill complete");
            }

            Ok(())
        }

        MaterializeCommands::List => {
            let rows = conn
                .query(
                    r#"
                    SELECT 
                        view_name,
                        view_definition
                    FROM timescaledb_information.continuous_aggregates
                    ORDER BY view_name
                    "#,
                    &[],
                )
                .await?;

            if rows.is_empty() {
                println!("No continuous aggregates found");
                return Ok(());
            }

            println!("{:<30} {}", "NAME", "DEFINITION");
            println!("{}", "-".repeat(80));

            for row in rows {
                let name: String = row.get(0);
                let def: String = row.get(1);
                let short_def: String = def.chars().take(50).collect();
                println!("{:<30} {}...", name, short_def.replace('\n', " "));
            }

            Ok(())
        }

        MaterializeCommands::Drop { name } => {
            println!("Dropping continuous aggregate '{}'...", name);

            let sql = format!("DROP MATERIALIZED VIEW IF EXISTS {} CASCADE", name);
            conn.execute(&sql, &[]).await?;

            println!("✓ Dropped");
            Ok(())
        }

        MaterializeCommands::Refresh { name, full } => {
            if full {
                println!("Refreshing all data for '{}'...", name);
                let sql = format!("CALL refresh_continuous_aggregate('{}', NULL, NULL)", name);
                conn.execute(&sql, &[]).await?;
            } else {
                println!("Refreshing recent data for '{}'...", name);
                let sql = format!(
                    "CALL refresh_continuous_aggregate('{}', NOW() - INTERVAL '24 hours', NOW())",
                    name
                );
                conn.execute(&sql, &[]).await?;
            }

            println!("✓ Refresh complete");
            Ok(())
        }

        MaterializeCommands::Info { name } => {
            let rows = conn
                .query(
                    r#"
                    SELECT 
                        ca.view_name,
                        ca.view_owner,
                        ca.materialization_hypertable_name,
                        j.schedule_interval,
                        j.config
                    FROM timescaledb_information.continuous_aggregates ca
                    LEFT JOIN timescaledb_information.jobs j 
                        ON j.hypertable_name = ca.materialization_hypertable_name
                    WHERE ca.view_name = $1
                    "#,
                    &[&name],
                )
                .await?;

            if rows.is_empty() {
                return Err(anyhow!("Continuous aggregate '{}' not found", name));
            }

            let row = &rows[0];
            let view_name: String = row.get(0);
            let owner: String = row.get(1);
            let hypertable: String = row.get(2);
            let interval: Option<String> = row.try_get::<_, String>(3).ok();

            println!("Name:       {}", view_name);
            println!("Owner:      {}", owner);
            println!("Hypertable: {}", hypertable);
            if let Some(i) = interval {
                println!("Refresh:    every {}", i);
            }

            let count_rows = conn
                .query(
                    &format!("SELECT COUNT(*) FROM {}", name),
                    &[],
                )
                .await?;
            let count: i64 = count_rows[0].get(0);
            println!("Rows:       {}", count);

            Ok(())
        }
    }
}
