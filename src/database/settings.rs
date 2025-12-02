use super::Database;
use poise::serenity_prelude::GuildId;
use sqlx::Error as SqlxError;

impl Database {
    /// Set timezone for a guild
    pub async fn set_guild_timezone(
        &self,
        guild_id: GuildId,
        timezone: String,
    ) -> Result<(), SqlxError> {
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
        .execute(self.pool())
        .await?;
        Ok(())
    }

    /// Get timezone for a guild (returns "UTC" if not set)
    pub async fn get_guild_timezone(&self, guild_id: GuildId) -> Result<String, SqlxError> {
        let result: Option<(String,)> = sqlx::query_as(
            "SELECT timezone FROM guild_settings WHERE guild_id = $1",
        )
        .bind(guild_id.get() as i64)
        .fetch_optional(self.pool())
        .await?;

        Ok(result.map(|(tz,)| tz).unwrap_or_else(|| "UTC".to_string()))
    }
}
