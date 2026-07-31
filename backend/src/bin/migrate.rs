//! `wara-migrate` — the single migration binary for Wara's database schema.
//!
//! Thin wrapper around Toasty's migration tooling (`toasty-cli`). It builds a
//! Toasty `Db` with every product model registered, loads migration settings
//! from `Toasty.toml`, and dispatches the requested subcommand:
//!
//! - `wara-migrate migration generate --name <name>` — author a new migration
//!   from the current model/schema diff.
//! - `wara-migrate migration apply` — apply pending migrations.
//! - `wara-migrate migration snapshot` — print the current schema snapshot.
//!
//! Run it from the `backend/` directory (the Makefile target and Docker image do)
//! so `Toasty.toml` and the `toasty/` migration directory resolve.

use toasty_cli::{Config as MigrationConfig, ToastyCli};
use wara_backend::{
    errors::wara::WaraError,
    libs::{config::Config, db},
};

#[tokio::main]
async fn main() -> Result<(), WaraError> {
    let config = Config::from_env();
    let db = db::build_toasty(&config).await?;
    let migration_config =
        MigrationConfig::load().map_err(|error| WaraError::ConfigFile(error.to_string()))?;
    ToastyCli::with_config(db, migration_config)
        .parse_and_run()
        .await
        .map_err(|error| WaraError::Database(error.to_string()))
}
