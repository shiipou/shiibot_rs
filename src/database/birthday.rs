use super::Database;
use poise::serenity_prelude::{ChannelId, GuildId, MessageId, RoleId, UserId};
use sqlx::Error as SqlxError;

impl Database {
    /// Save or update a user's birthday
    pub async fn upsert_birthday(
        &self,
        user_id: UserId,
        month: i32,
        day: i32,
        year: Option<i32>,
    ) -> Result<(), SqlxError> {
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
        .execute(self.pool())
        .await?;
        Ok(())
    }

    /// Get a user's birthday
    pub async fn get_birthday(
        &self,
        user_id: UserId,
    ) -> Result<Option<(i32, i32, Option<i32>)>, SqlxError> {
        let result: Option<(i32, i32, Option<i32>)> = sqlx::query_as(
            "SELECT birth_month, birth_day, birth_year FROM user_birthdays WHERE user_id = $1",
        )
        .bind(user_id.get() as i64)
        .fetch_optional(self.pool())
        .await?;

        Ok(result)
    }

    /// Get all users with birthdays on a specific date
    pub async fn get_birthdays_on_date(
        &self,
        month: i32,
        day: i32,
    ) -> Result<Vec<(UserId, Option<i32>)>, SqlxError> {
        let rows: Vec<(i64, Option<i32>)> = sqlx::query_as(
            "SELECT user_id, birth_year FROM user_birthdays WHERE birth_month = $1 AND birth_day = $2",
        )
        .bind(month)
        .bind(day)
        .fetch_all(self.pool())
        .await?;

        Ok(rows
            .into_iter()
            .map(|(user_id, year)| (UserId::new(user_id as u64), year))
            .collect())
    }

    /// Set birthday notification channel for a guild
    #[allow(clippy::too_many_arguments)]
    pub async fn set_birthday_channel(
        &self,
        guild_id: GuildId,
        channel_id: ChannelId,
        message_id: Option<MessageId>,
        birthday_role_id: Option<RoleId>,
        custom_message: Option<String>,
        custom_message_without_age: Option<String>,
        custom_header: Option<String>,
        custom_footer: Option<String>,
        collection_message_title: Option<String>,
        collection_message_description: Option<String>,
        collection_button_label: Option<String>,
    ) -> Result<(), SqlxError> {
        sqlx::query(
            r#"
            INSERT INTO birthday_channels (
                guild_id, channel_id, message_id, birthday_role_id, 
                custom_message, custom_message_without_age, custom_header, custom_footer, 
                collection_message_title, collection_message_description, collection_button_label
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
            ON CONFLICT (guild_id) 
            DO UPDATE SET 
                channel_id = $2,
                message_id = $3,
                birthday_role_id = $4,
                custom_message = $5,
                custom_message_without_age = $6,
                custom_header = $7,
                custom_footer = $8,
                collection_message_title = $9,
                collection_message_description = $10,
                collection_button_label = $11
            "#,
        )
        .bind(guild_id.get() as i64)
        .bind(channel_id.get() as i64)
        .bind(message_id.map(|id| id.get() as i64))
        .bind(birthday_role_id.map(|id| id.get() as i64))
        .bind(custom_message)
        .bind(custom_message_without_age)
        .bind(custom_header)
        .bind(custom_footer)
        .bind(collection_message_title)
        .bind(collection_message_description)
        .bind(collection_button_label)
        .execute(self.pool())
        .await?;
        Ok(())
    }

    /// Get birthday notification channel for a guild
    pub async fn get_birthday_channel(
        &self,
        guild_id: GuildId,
    ) -> Result<
        Option<(ChannelId, Option<MessageId>, Option<String>, Option<String>, Option<String>, Option<String>)>,
        SqlxError,
    > {
        let result: Option<(i64, Option<i64>, Option<String>, Option<String>, Option<String>, Option<String>)> =
            sqlx::query_as(
                "SELECT channel_id, message_id, custom_message, custom_message_without_age, custom_header, custom_footer \
                 FROM birthday_channels WHERE guild_id = $1",
            )
            .bind(guild_id.get() as i64)
            .fetch_optional(self.pool())
            .await?;

        Ok(result.map(|(channel_id, message_id, msg, msg_without_age, header, footer)| {
            (
                ChannelId::new(channel_id as u64),
                message_id.map(|id| MessageId::new(id as u64)),
                msg,
                msg_without_age,
                header,
                footer,
            )
        }))
    }

    /// Get birthday role for a guild
    pub async fn get_birthday_role(&self, guild_id: GuildId) -> Result<Option<RoleId>, SqlxError> {
        let result: Option<(i64,)> = sqlx::query_as(
            "SELECT birthday_role_id FROM birthday_channels \
             WHERE guild_id = $1 AND birthday_role_id IS NOT NULL",
        )
        .bind(guild_id.get() as i64)
        .fetch_optional(self.pool())
        .await?;

        Ok(result.map(|(role_id,)| RoleId::new(role_id as u64)))
    }

    /// Get birthday collection message configuration for a guild
    pub async fn get_birthday_collection_config(
        &self,
        guild_id: GuildId,
    ) -> Result<Option<(Option<String>, Option<String>, Option<String>)>, SqlxError> {
        let result: Option<(Option<String>, Option<String>, Option<String>)> = sqlx::query_as(
            "SELECT collection_message_title, collection_message_description, collection_button_label \
             FROM birthday_channels WHERE guild_id = $1",
        )
        .bind(guild_id.get() as i64)
        .fetch_optional(self.pool())
        .await?;

        Ok(result)
    }

    /// Remove birthday notification channel for a guild
    /// Returns (channel_id, message_id) if a record was deleted
    pub async fn remove_birthday_channel(
        &self,
        guild_id: GuildId,
    ) -> Result<Option<(ChannelId, Option<MessageId>)>, SqlxError> {
        let result: Option<(i64, Option<i64>)> = sqlx::query_as(
            "DELETE FROM birthday_channels WHERE guild_id = $1 RETURNING channel_id, message_id",
        )
        .bind(guild_id.get() as i64)
        .fetch_optional(self.pool())
        .await?;

        Ok(result.map(|(channel_id, message_id)| {
            (
                ChannelId::new(channel_id as u64),
                message_id.map(|id| MessageId::new(id as u64)),
            )
        }))
    }

    /// Check if any birthday channels are configured
    pub async fn has_any_birthday_channels(&self) -> Result<bool, SqlxError> {
        let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM birthday_channels")
            .fetch_one(self.pool())
            .await?;

        Ok(count.0 > 0)
    }
}
