/// Database modules organized by feature
mod migrations;
mod lobby;
mod birthday;
mod schedule;
mod settings;

use sqlx::{PgPool, postgres::PgPoolOptions};
use tracing::info;

/// Database connection pool wrapper
///
/// Handles all database operations for the bot
#[derive(Clone)]
pub struct Database {
    pool: PgPool,
}

impl Database {
    /// Create a new database connection and run migrations
    pub async fn new(database_url: &str) -> Result<Self, sqlx::Error> {
        let pool = PgPoolOptions::new()
            .max_connections(5)
            .connect(database_url)
            .await?;

        let db = Self { pool };
        db.run_migrations().await?;

        info!("Database connected and migrations completed");
        Ok(db)
    }

    /// Get a reference to the connection pool (for internal use)
    pub(crate) fn pool(&self) -> &PgPool {
        &self.pool
    }
}
