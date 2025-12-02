/// Type of scheduled task
#[derive(Debug, Clone, PartialEq, Eq, sqlx::Type)]
#[sqlx(type_name = "schedule_type", rename_all = "lowercase")]
pub enum ScheduleType {
    Birthday,
    BirthdayRole,
}

/// A scheduled task configuration
#[derive(Debug, Clone)]
pub struct Schedule {
    pub id: i32,
    pub guild_id: Option<i64>, // None means it runs for all guilds (e.g., BirthdayRole)
    pub schedule_type: ScheduleType,
    pub cron_expression: String, // Cron expression (e.g., "0 0 8 * * *" for 8 AM daily)
    pub enabled: bool,
}
