use super::Database;
use sqlx::Error as SqlxError;

impl Database {
    /// Run database migrations to create tables
    pub(super) async fn run_migrations(&self) -> Result<(), SqlxError> {
        self.create_lobby_tables().await?;
        self.create_guild_settings_table().await?;
        self.create_birthday_tables().await?;
        self.create_schedule_tables().await?;
        Ok(())
    }

    async fn create_lobby_tables(&self) -> Result<(), SqlxError> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS lobby_channels (
                channel_id BIGINT PRIMARY KEY,
                guild_id BIGINT NOT NULL
            )
            "#,
        )
        .execute(self.pool())
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
        .execute(self.pool())
        .await?;

        // Add columns if they don't exist (for existing databases)
        sqlx::query(
            r#"
            DO $$
            BEGIN
                IF NOT EXISTS (SELECT 1 FROM information_schema.columns 
                              WHERE table_name = 'temp_channels' AND column_name = 'is_persistent') THEN
                    ALTER TABLE temp_channels ADD COLUMN is_persistent BOOLEAN NOT NULL DEFAULT FALSE;
                END IF;
                IF NOT EXISTS (SELECT 1 FROM information_schema.columns 
                              WHERE table_name = 'temp_channels' AND column_name = 'is_archived') THEN
                    ALTER TABLE temp_channels ADD COLUMN is_archived BOOLEAN NOT NULL DEFAULT FALSE;
                END IF;
            END $$;
            "#,
        )
        .execute(self.pool())
        .await?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS archive_categories (
                guild_id BIGINT PRIMARY KEY,
                category_id BIGINT NOT NULL
            )
            "#,
        )
        .execute(self.pool())
        .await?;

        Ok(())
    }

    async fn create_guild_settings_table(&self) -> Result<(), SqlxError> {
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
        .execute(self.pool())
        .await?;

        Ok(())
    }

    async fn create_birthday_tables(&self) -> Result<(), SqlxError> {
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
        .execute(self.pool())
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
        .execute(self.pool())
        .await?;

        // Add columns if they don't exist (for existing databases)
        sqlx::query(
            r#"
            DO $$
            BEGIN
                IF NOT EXISTS (SELECT 1 FROM information_schema.columns 
                              WHERE table_name = 'birthday_channels' AND column_name = 'message_id') THEN
                    ALTER TABLE birthday_channels ADD COLUMN message_id BIGINT;
                END IF;
                IF NOT EXISTS (SELECT 1 FROM information_schema.columns 
                              WHERE table_name = 'birthday_channels' AND column_name = 'birthday_role_id') THEN
                    ALTER TABLE birthday_channels ADD COLUMN birthday_role_id BIGINT;
                END IF;
                IF NOT EXISTS (SELECT 1 FROM information_schema.columns 
                              WHERE table_name = 'birthday_channels' AND column_name = 'custom_header') THEN
                    ALTER TABLE birthday_channels ADD COLUMN custom_header TEXT;
                END IF;
                IF NOT EXISTS (SELECT 1 FROM information_schema.columns 
                              WHERE table_name = 'birthday_channels' AND column_name = 'custom_footer') THEN
                    ALTER TABLE birthday_channels ADD COLUMN custom_footer TEXT;
                END IF;
                IF NOT EXISTS (SELECT 1 FROM information_schema.columns 
                              WHERE table_name = 'birthday_channels' AND column_name = 'collection_message_title') THEN
                    ALTER TABLE birthday_channels ADD COLUMN collection_message_title TEXT;
                END IF;
                IF NOT EXISTS (SELECT 1 FROM information_schema.columns 
                              WHERE table_name = 'birthday_channels' AND column_name = 'collection_message_description') THEN
                    ALTER TABLE birthday_channels ADD COLUMN collection_message_description TEXT;
                END IF;
                IF NOT EXISTS (SELECT 1 FROM information_schema.columns 
                              WHERE table_name = 'birthday_channels' AND column_name = 'collection_button_label') THEN
                    ALTER TABLE birthday_channels ADD COLUMN collection_button_label TEXT;
                END IF;
                IF NOT EXISTS (SELECT 1 FROM information_schema.columns 
                              WHERE table_name = 'birthday_channels' AND column_name = 'custom_message_without_age') THEN
                    ALTER TABLE birthday_channels ADD COLUMN custom_message_without_age TEXT;
                END IF;
            END $$;
            "#,
        )
        .execute(self.pool())
        .await?;

        Ok(())
    }

    async fn create_schedule_tables(&self) -> Result<(), SqlxError> {
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
        .execute(self.pool())
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
        .execute(self.pool())
        .await?;

        Ok(())
    }
}
