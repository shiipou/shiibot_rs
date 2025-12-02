use super::Database;
use poise::serenity_prelude::GuildId;
use sqlx::Error as SqlxError;

impl Database {
    /// Get all schedules from the database
    pub async fn get_all_schedules(&self) -> Result<Vec<crate::schedule::Schedule>, SqlxError> {
        let rows: Vec<(i32, Option<i64>, crate::schedule::ScheduleType, String, bool)> =
            sqlx::query_as(
                "SELECT id, guild_id, schedule_type, cron_expression, enabled FROM schedules",
            )
            .fetch_all(self.pool())
            .await?;

        Ok(rows
            .into_iter()
            .map(
                |(id, guild_id, schedule_type, cron_expression, enabled)| {
                    crate::schedule::Schedule {
                        id,
                        guild_id,
                        schedule_type,
                        cron_expression,
                        enabled,
                    }
                },
            )
            .collect())
    }

    /// Create or update a schedule
    pub async fn upsert_schedule(
        &self,
        guild_id: Option<GuildId>,
        schedule_type: crate::schedule::ScheduleType,
        cron_expression: String,
        enabled: bool,
    ) -> Result<(), SqlxError> {
        let guild_id_value = guild_id.map(|id| id.get() as i64);

        // Check if a schedule of this type already exists for this guild
        let existing: Option<(i32,)> = if let Some(gid) = guild_id_value {
            sqlx::query_as(
                "SELECT id FROM schedules WHERE guild_id = $1 AND schedule_type = $2",
            )
            .bind(gid)
            .bind(&schedule_type)
            .fetch_optional(self.pool())
            .await?
        } else {
            sqlx::query_as(
                "SELECT id FROM schedules WHERE guild_id IS NULL AND schedule_type = $1",
            )
            .bind(&schedule_type)
            .fetch_optional(self.pool())
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
                .execute(self.pool())
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
                .execute(self.pool())
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
            .execute(self.pool())
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
    ) -> Result<(), SqlxError> {
        if let Some(gid) = guild_id {
            sqlx::query(
                "UPDATE schedules SET enabled = $1, updated_at = NOW() \
                 WHERE guild_id = $2 AND schedule_type = $3",
            )
            .bind(enabled)
            .bind(gid.get() as i64)
            .bind(schedule_type)
            .execute(self.pool())
            .await?;
        } else {
            sqlx::query(
                "UPDATE schedules SET enabled = $1, updated_at = NOW() \
                 WHERE guild_id IS NULL AND schedule_type = $2",
            )
            .bind(enabled)
            .bind(schedule_type)
            .execute(self.pool())
            .await?;
        }

        Ok(())
    }
}
