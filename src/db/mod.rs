mod pool;
mod schema;
mod views;

pub use pool::create_pool;
pub use schema::run_migrations;
pub use views::{list_materialized_views, refresh_materialized_views};

pub type Pool = deadpool_postgres::Pool;
