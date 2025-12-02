use poise::serenity_prelude::{ChannelId, GuildId, UserId};
use sqlx::{PgPool, postgres::PgPoolOptions};
use tracing::info;

/// Database connection pool wrapper
///
/// Handles all database operations for the bot including lobby channels,
/// temporary channels, and archive categories.
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

        // Guild settings table
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS guild_settings (
                guild_id BIGINT PRIMARY KEY,
                timezone TEXT NOT NULL DEFAULT 'UTC',
                created_at TIMESTAMP NOT NULL DEFAULT NOW(),
                updated_at TIMESTAMP NOT NULL DEFAULT NOW()
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        // Birthday tables
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS user_birthdays (
                user_id BIGINT PRIMARY KEY,
                birth_month INTEGER NOT NULL CHECK (birth_month BETWEEN 1 AND 12),
                birth_day INTEGER NOT NULL CHECK (birth_day BETWEEN 1 AND 31),
                birth_year INTEGER CHECK (birth_year IS NULL OR birth_year > 1900),
                created_at TIMESTAMP NOT NULL DEFAULT NOW(),
                updated_at TIMESTAMP NOT NULL DEFAULT NOW()
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS birthday_channels (
                guild_id BIGINT PRIMARY KEY,
                channel_id BIGINT NOT NULL,
                message_id BIGINT,
                birthday_role_id BIGINT,
                custom_message TEXT,
                custom_header TEXT,
                custom_footer TEXT,
                collection_message_title TEXT,
                collection_message_description TEXT,
                collection_button_label TEXT,
                created_at TIMESTAMP NOT NULL DEFAULT NOW()
            )
            "#,
        )
        .execute(&self.pool)
        .await?;


        // Create schedule_type enum if it doesn't exist
        sqlx::query(
            r#"
            DO $$ BEGIN
                CREATE TYPE schedule_type AS ENUM ('birthday', 'birthdayrole');
            EXCEPTION
                WHEN duplicate_object THEN 
                    -- Type already exists, try to add new values if they don't exist
                    ALTER TYPE schedule_type ADD VALUE IF NOT EXISTS 'birthdayrole';
            END $$;
            "#,
        )
        .execute(&self.pool)
        .await?;

        // Schedules table
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS schedules (
                id SERIAL PRIMARY KEY,
                guild_id BIGINT,
                schedule_type schedule_type NOT NULL,
                cron_expression TEXT NOT NULL,
                enabled BOOLEAN NOT NULL DEFAULT TRUE,
                created_at TIMESTAMP NOT NULL DEFAULT NOW(),
                updated_at TIMESTAMP NOT NULL DEFAULT NOW()
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

    // Birthday-related methods

    /// Save or update a user's birthday
    pub async fn upsert_birthday(
        &self,
        user_id: UserId,
        month: i32,
        day: i32,
        year: Option<i32>,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            INSERT INTO user_birthdays (user_id, birth_month, birth_day, birth_year, updated_at)
            VALUES ($1, $2, $3, $4, NOW())
            ON CONFLICT (user_id) 
            DO UPDATE SET 
                birth_month = $2, 
                birth_day = $3, 
                birth_year = $4,
                updated_at = NOW()
            "#,
        )
        .bind(user_id.get() as i64)
        .bind(month)
        .bind(day)
        .bind(year)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Get a user's birthday
    pub async fn get_birthday(
        &self,
        user_id: UserId,
    ) -> Result<Option<(i32, i32, Option<i32>)>, sqlx::Error> {
        let result: Option<(i32, i32, Option<i32>)> = sqlx::query_as(
            "SELECT birth_month, birth_day, birth_year FROM user_birthdays WHERE user_id = $1",
        )
        .bind(user_id.get() as i64)
        .fetch_optional(&self.pool)
        .await?;

        Ok(result)
    }

    /// Set birthday notification channel for a guild
    pub async fn set_birthday_channel(
        &self,
        guild_id: GuildId,
        channel_id: ChannelId,
        message_id: Option<poise::serenity_prelude::MessageId>,
        birthday_role_id: Option<poise::serenity_prelude::RoleId>,
        custom_message: Option<String>,
        custom_header: Option<String>,
        custom_footer: Option<String>,
        collection_message_title: Option<String>,
        collection_message_description: Option<String>,
        collection_button_label: Option<String>,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            INSERT INTO birthday_channels (guild_id, channel_id, message_id, birthday_role_id, custom_message, custom_header, custom_footer, collection_message_title, collection_message_description, collection_button_label)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            ON CONFLICT (guild_id) 
            DO UPDATE SET 
                channel_id = $2,
                message_id = $3,
                birthday_role_id = $4,
                custom_message = $5,
                custom_header = $6,
                custom_footer = $7,
                collection_message_title = $8,
                collection_message_description = $9,
                collection_button_label = $10
            "#,
        )
        .bind(guild_id.get() as i64)
        .bind(channel_id.get() as i64)
        .bind(message_id.map(|id| id.get() as i64))
        .bind(birthday_role_id.map(|id| id.get() as i64))
        .bind(custom_message)
        .bind(custom_header)
        .bind(custom_footer)
        .bind(collection_message_title)
        .bind(collection_message_description)
        .bind(collection_button_label)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Get birthday notification channel for a guild
    pub async fn get_birthday_channel(
        &self,
        guild_id: GuildId,
    ) -> Result<Option<(ChannelId, Option<poise::serenity_prelude::MessageId>, Option<String>, Option<String>, Option<String>)>, sqlx::Error> {
        let result: Option<(i64, Option<i64>, Option<String>, Option<String>, Option<String>)> = sqlx::query_as(
            "SELECT channel_id, message_id, custom_message, custom_header, custom_footer FROM birthday_channels WHERE guild_id = $1",
        )
        .bind(guild_id.get() as i64)
        .fetch_optional(&self.pool)
        .await?;

        Ok(result.map(|(channel_id, message_id, msg, header, footer)| (
            ChannelId::new(channel_id as u64),
            message_id.map(|id| poise::serenity_prelude::MessageId::new(id as u64)),
            msg,
            header,
            footer
        )))
    }

    /// Get birthday role for a guild
    pub async fn get_birthday_role(
        &self,
        guild_id: GuildId,
    ) -> Result<Option<poise::serenity_prelude::RoleId>, sqlx::Error> {
        let result: Option<(i64,)> = sqlx::query_as(
            "SELECT birthday_role_id FROM birthday_channels WHERE guild_id = $1 AND birthday_role_id IS NOT NULL",
        )
        .bind(guild_id.get() as i64)
        .fetch_optional(&self.pool)
        .await?;

        Ok(result.map(|(role_id,)| poise::serenity_prelude::RoleId::new(role_id as u64)))
    }

    /// Get birthday collection message configuration for a guild
    pub async fn get_birthday_collection_config(
        &self,
        guild_id: GuildId,
    ) -> Result<Option<(Option<String>, Option<String>, Option<String>)>, sqlx::Error> {
        let result: Option<(Option<String>, Option<String>, Option<String>)> = sqlx::query_as(
            "SELECT collection_message_title, collection_message_description, collection_button_label FROM birthday_channels WHERE guild_id = $1",
        )
        .bind(guild_id.get() as i64)
        .fetch_optional(&self.pool)
        .await?;

        Ok(result)
    }

    /// Remove birthday notification channel for a guild
    /// Returns (channel_id, message_id) if a record was deleted
    pub async fn remove_birthday_channel(
        &self,
        guild_id: GuildId,
    ) -> Result<Option<(ChannelId, Option<poise::serenity_prelude::MessageId>)>, sqlx::Error> {
        let result: Option<(i64, Option<i64>)> = sqlx::query_as(
            "DELETE FROM birthday_channels WHERE guild_id = $1 RETURNING channel_id, message_id",
        )
        .bind(guild_id.get() as i64)
        .fetch_optional(&self.pool)
        .await?;

        Ok(result.map(|(channel_id, message_id)| (
            ChannelId::new(channel_id as u64),
            message_id.map(|id| poise::serenity_prelude::MessageId::new(id as u64)),
        )))
    }

    /// Get all users with birthdays on a specific date
    pub async fn get_birthdays_on_date(
        &self,
        month: i32,
        day: i32,
    ) -> Result<Vec<(UserId, Option<i32>)>, sqlx::Error> {
        let rows: Vec<(i64, Option<i32>)> = sqlx::query_as(
            "SELECT user_id, birth_year FROM user_birthdays WHERE birth_month = $1 AND birth_day = $2",
        )
        .bind(month)
        .bind(day)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|(user_id, year)| (UserId::new(user_id as u64), year))
            .collect())
    }

    // Schedule-related methods

    /// Get all schedules from the database
    pub async fn get_all_schedules(&self) -> Result<Vec<crate::schedule::Schedule>, sqlx::Error> {
        let rows: Vec<(i32, Option<i64>, crate::schedule::ScheduleType, String, bool)> = sqlx::query_as(
            "SELECT id, guild_id, schedule_type, cron_expression, enabled FROM schedules",
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|(id, guild_id, schedule_type, cron_expression, enabled)| crate::schedule::Schedule {
                id,
                guild_id,
                schedule_type,
                cron_expression,
                enabled,
            })
            .collect())
    }

    /// Create or update a schedule
    pub async fn upsert_schedule(
        &self,
        guild_id: Option<GuildId>,
        schedule_type: crate::schedule::ScheduleType,
        cron_expression: String,
        enabled: bool,
    ) -> Result<(), sqlx::Error> {
        let guild_id_value = guild_id.map(|id| id.get() as i64);
        
        // Check if a schedule of this type already exists for this guild (or globally if guild_id is None)
        let existing: Option<(i32,)> = if let Some(gid) = guild_id_value {
            sqlx::query_as(
                "SELECT id FROM schedules WHERE guild_id = $1 AND schedule_type = $2",
            )
            .bind(gid)
            .bind(&schedule_type)
            .fetch_optional(&self.pool)
            .await?
        } else {
            sqlx::query_as(
                "SELECT id FROM schedules WHERE guild_id IS NULL AND schedule_type = $1",
            )
            .bind(&schedule_type)
            .fetch_optional(&self.pool)
            .await?
        };

        if existing.is_some() {
            // Update existing schedule
            if let Some(gid) = guild_id_value {
                sqlx::query(
                    r#"
                    UPDATE schedules 
                    SET cron_expression = $1, enabled = $2, updated_at = NOW()
                    WHERE guild_id = $3 AND schedule_type = $4
                    "#,
                )
                .bind(&cron_expression)
                .bind(enabled)
                .bind(gid)
                .bind(schedule_type)
                .execute(&self.pool)
                .await?;
            } else {
                sqlx::query(
                    r#"
                    UPDATE schedules 
                    SET cron_expression = $1, enabled = $2, updated_at = NOW()
                    WHERE guild_id IS NULL AND schedule_type = $3
                    "#,
                )
                .bind(&cron_expression)
                .bind(enabled)
                .bind(schedule_type)
                .execute(&self.pool)
                .await?;
            }
        } else {
            // Insert new schedule
            sqlx::query(
                r#"
                INSERT INTO schedules (guild_id, schedule_type, cron_expression, enabled)
                VALUES ($1, $2, $3, $4)
                "#,
            )
            .bind(guild_id_value)
            .bind(schedule_type)
            .bind(&cron_expression)
            .bind(enabled)
            .execute(&self.pool)
            .await?;
        }

        Ok(())
    }

    /// Enable or disable a schedule by type for a specific guild (or globally if guild_id is None)
    pub async fn set_schedule_enabled(
        &self,
        guild_id: Option<GuildId>,
        schedule_type: crate::schedule::ScheduleType,
        enabled: bool,
    ) -> Result<(), sqlx::Error> {
        if let Some(gid) = guild_id {
            sqlx::query(
                "UPDATE schedules SET enabled = $1, updated_at = NOW() WHERE guild_id = $2 AND schedule_type = $3",
            )
            .bind(enabled)
            .bind(gid.get() as i64)
            .bind(schedule_type)
            .execute(&self.pool)
            .await?;
        } else {
            sqlx::query(
                "UPDATE schedules SET enabled = $1, updated_at = NOW() WHERE guild_id IS NULL AND schedule_type = $2",
            )
            .bind(enabled)
            .bind(schedule_type)
            .execute(&self.pool)
            .await?;
        }

        Ok(())
    }

    /// Check if any birthday channels are configured
    pub async fn has_any_birthday_channels(&self) -> Result<bool, sqlx::Error> {
        let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM birthday_channels")
            .fetch_one(&self.pool)
            .await?;

        Ok(count.0 > 0)
    }

    /// Set timezone for a guild
    pub async fn set_guild_timezone(&self, guild_id: GuildId, timezone: String) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            INSERT INTO guild_settings (guild_id, timezone, updated_at)
            VALUES ($1, $2, NOW())
            ON CONFLICT (guild_id)
            DO UPDATE SET timezone = $2, updated_at = NOW()
            "#,
        )
        .bind(guild_id.get() as i64)
        .bind(timezone)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Get timezone for a guild (returns "UTC" if not set)
    pub async fn get_guild_timezone(&self, guild_id: GuildId) -> Result<String, sqlx::Error> {
        let result: Option<(String,)> = sqlx::query_as(
            "SELECT timezone FROM guild_settings WHERE guild_id = $1",
        )
        .bind(guild_id.get() as i64)
        .fetch_optional(&self.pool)
        .await?;

        Ok(result.map(|(tz,)| tz).unwrap_or_else(|| "UTC".to_string()))
    }
}
