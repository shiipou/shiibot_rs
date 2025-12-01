use poise::serenity_prelude::{ChannelId, GuildId, UserId};
use sqlx::{PgPool, postgres::PgPoolOptions};
use tracing::info;

/// Database connection pool wrapper
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

    /// Run database migrations to create tables
    async fn run_migrations(&self) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS lobby_channels (
                channel_id BIGINT PRIMARY KEY,
                guild_id BIGINT NOT NULL
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS temp_channels (
                channel_id BIGINT PRIMARY KEY,
                guild_id BIGINT NOT NULL,
                owner_id BIGINT NOT NULL,
                lobby_channel_id BIGINT NOT NULL,
                is_persistent BOOLEAN NOT NULL DEFAULT FALSE,
                is_archived BOOLEAN NOT NULL DEFAULT FALSE
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        // Add columns if they don't exist (for existing databases)
        sqlx::query(
            r#"
            DO $$
            BEGIN
                IF NOT EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = 'temp_channels' AND column_name = 'is_persistent') THEN
                    ALTER TABLE temp_channels ADD COLUMN is_persistent BOOLEAN NOT NULL DEFAULT FALSE;
                END IF;
                IF NOT EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = 'temp_channels' AND column_name = 'is_archived') THEN
                    ALTER TABLE temp_channels ADD COLUMN is_archived BOOLEAN NOT NULL DEFAULT FALSE;
                END IF;
            END $$;
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS archive_categories (
                guild_id BIGINT PRIMARY KEY,
                category_id BIGINT NOT NULL
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Insert a lobby channel into the database
    pub async fn insert_lobby_channel(
        &self,
        channel_id: ChannelId,
        guild_id: GuildId,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO lobby_channels (channel_id, guild_id) VALUES ($1, $2) ON CONFLICT (channel_id) DO NOTHING",
        )
        .bind(channel_id.get() as i64)
        .bind(guild_id.get() as i64)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Get all lobby channels
    pub async fn get_all_lobby_channels(&self) -> Result<Vec<(ChannelId, GuildId)>, sqlx::Error> {
        let rows: Vec<(i64, i64)> =
            sqlx::query_as("SELECT channel_id, guild_id FROM lobby_channels")
                .fetch_all(&self.pool)
                .await?;

        Ok(rows
            .into_iter()
            .map(|(channel_id, guild_id)| {
                (
                    ChannelId::new(channel_id as u64),
                    GuildId::new(guild_id as u64),
                )
            })
            .collect())
    }

    /// Remove a lobby channel from the database
    #[allow(dead_code)]
    pub async fn remove_lobby_channel(&self, channel_id: ChannelId) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM lobby_channels WHERE channel_id = $1")
            .bind(channel_id.get() as i64)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Insert a temp channel into the database
    pub async fn insert_temp_channel(
        &self,
        channel_id: ChannelId,
        guild_id: GuildId,
        owner_id: UserId,
        lobby_channel_id: ChannelId,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO temp_channels (channel_id, guild_id, owner_id, lobby_channel_id, is_persistent, is_archived) VALUES ($1, $2, $3, $4, FALSE, FALSE) ON CONFLICT (channel_id) DO NOTHING",
        )
        .bind(channel_id.get() as i64)
        .bind(guild_id.get() as i64)
        .bind(owner_id.get() as i64)
        .bind(lobby_channel_id.get() as i64)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Get all temp channels (including persistent and archived status)
    pub async fn get_all_temp_channels(
        &self,
    ) -> Result<Vec<(ChannelId, GuildId, UserId, ChannelId, bool, bool)>, sqlx::Error> {
        let rows: Vec<(i64, i64, i64, i64, bool, bool)> = sqlx::query_as(
            "SELECT channel_id, guild_id, owner_id, lobby_channel_id, is_persistent, is_archived FROM temp_channels",
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(
                |(channel_id, guild_id, owner_id, lobby_channel_id, is_persistent, is_archived)| {
                    (
                        ChannelId::new(channel_id as u64),
                        GuildId::new(guild_id as u64),
                        UserId::new(owner_id as u64),
                        ChannelId::new(lobby_channel_id as u64),
                        is_persistent,
                        is_archived,
                    )
                },
            )
            .collect())
    }

    /// Remove a temp channel from the database
    pub async fn remove_temp_channel(&self, channel_id: ChannelId) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM temp_channels WHERE channel_id = $1")
            .bind(channel_id.get() as i64)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Set a temp channel as persistent
    pub async fn set_channel_persistent(
        &self,
        channel_id: ChannelId,
        is_persistent: bool,
    ) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE temp_channels SET is_persistent = $1 WHERE channel_id = $2")
            .bind(is_persistent)
            .bind(channel_id.get() as i64)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Set a temp channel as archived
    pub async fn set_channel_archived(
        &self,
        channel_id: ChannelId,
        is_archived: bool,
    ) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE temp_channels SET is_archived = $1 WHERE channel_id = $2")
            .bind(is_archived)
            .bind(channel_id.get() as i64)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Get archived channel for a user from a specific lobby in a guild
    pub async fn get_archived_channel_for_user(
        &self,
        guild_id: GuildId,
        owner_id: UserId,
        lobby_channel_id: ChannelId,
    ) -> Result<Option<ChannelId>, sqlx::Error> {
        let result: Option<(i64,)> = sqlx::query_as(
            "SELECT channel_id FROM temp_channels WHERE guild_id = $1 AND owner_id = $2 AND lobby_channel_id = $3 AND is_archived = TRUE LIMIT 1",
        )
        .bind(guild_id.get() as i64)
        .bind(owner_id.get() as i64)
        .bind(lobby_channel_id.get() as i64)
        .fetch_optional(&self.pool)
        .await?;

        Ok(result.map(|(channel_id,)| ChannelId::new(channel_id as u64)))
    }

    /// Get or create archive category for a guild
    pub async fn get_archive_category(
        &self,
        guild_id: GuildId,
    ) -> Result<Option<ChannelId>, sqlx::Error> {
        let result: Option<(i64,)> =
            sqlx::query_as("SELECT category_id FROM archive_categories WHERE guild_id = $1")
                .bind(guild_id.get() as i64)
                .fetch_optional(&self.pool)
                .await?;

        Ok(result.map(|(category_id,)| ChannelId::new(category_id as u64)))
    }

    /// Set archive category for a guild
    pub async fn set_archive_category(
        &self,
        guild_id: GuildId,
        category_id: ChannelId,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO archive_categories (guild_id, category_id) VALUES ($1, $2) ON CONFLICT (guild_id) DO UPDATE SET category_id = $2",
        )
        .bind(guild_id.get() as i64)
        .bind(category_id.get() as i64)
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}
