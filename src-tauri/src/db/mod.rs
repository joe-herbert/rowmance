/// Local SQLite database — connection pool initialisation and migration runner.
use sqlx::{sqlite::SqlitePoolOptions, SqlitePool};
use std::path::PathBuf;

pub mod models;

/// Return the path to the local SQLite database, creating the parent
/// directory if it does not exist.
pub fn db_path() -> anyhow::Result<PathBuf> {
    let config_dir = dirs_or_fallback();
    let dir = config_dir.join("rowmance");
    std::fs::create_dir_all(&dir)?;
    Ok(dir.join("rowmance.db"))
}

/// Initialise the SQLite pool and run all pending migrations.
pub async fn init_pool() -> anyhow::Result<SqlitePool> {
    let path = db_path()?;
    let url = format!("sqlite://{}?mode=rwc", path.display());

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect(&url)
        .await?;

    sqlx::migrate!("src/db/migrations").run(&pool).await?;

    Ok(pool)
}

/// Return the platform config directory, falling back to $HOME/.config on
/// platforms where dirs cannot determine it.
fn dirs_or_fallback() -> PathBuf {
    // Use $HOME/.config on all platforms for consistent behaviour in CI.
    if let Some(home) = std::env::var_os("HOME") {
        return PathBuf::from(home).join(".config");
    }
    PathBuf::from(".")
}
