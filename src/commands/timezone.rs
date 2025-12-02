use tracing::{error, info};

use crate::{
    models::{Context, Error},
    utils::timezone::parse_timezone,
    utils::messages::{format_error, format_success},
    utils::validation::require_guild,
};

/// Set the timezone for this server
#[poise::command(slash_command, required_permissions = "MANAGE_GUILD")]
pub async fn setup_timezone(
    ctx: Context<'_>,
    #[description = "Timezone (e.g., Europe/Paris, America/New_York, Asia/Tokyo)"]
    timezone: String,
) -> Result<(), Error> {
    let guild_id = require_guild(ctx.guild_id())?;

    // Validate timezone using utility function
    let tz = match parse_timezone(&timezone) {
        Ok(tz) => tz,
        Err(_) => {
            ctx.say(format!(
                "{}\nPlease use a valid IANA timezone name like:\n\
                • Europe/Paris\n\
                • America/New_York\n\
                • Asia/Tokyo\n\
                • UTC\n\
                \n\
                You can find a full list at: https://en.wikipedia.org/wiki/List_of_tz_database_time_zones",
                format_error(&format!("Invalid timezone: '{}'", timezone))
            ))
            .await?;
            return Ok(());
        }
    };

    // Save timezone to database
    if let Err(e) = ctx
        .data()
        .db
        .set_guild_timezone(guild_id, timezone.clone())
        .await
    {
        error!("Failed to save guild timezone: {}", e);
        ctx.say(format_error("Failed to save timezone setting!")).await?;
        return Ok(());
    }

    // Show current time in the selected timezone
    let now = chrono::Utc::now().with_timezone(&tz);

    ctx.say(format!(
        "{}\nTimezone: **{}**\nCurrent time: **{}**\n\
        \nAll time-based commands (like `/setup_birthday`) will now use this timezone.",
        format_success("Server timezone configured!"),
        timezone,
        now.format("%Y-%m-%d %H:%M:%S %Z")
    ))
    .await?;

    info!("Set timezone for guild {} to {}", guild_id, timezone);

    Ok(())
}
