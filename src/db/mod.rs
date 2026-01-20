mod duckdb;
mod pool;
mod schema;

pub use self::duckdb::{execute_query as execute_duckdb_query, DuckDbPool};
pub use pool::create_pool;
pub use schema::run_migrations;

pub type Pool = deadpool_postgres::Pool;
