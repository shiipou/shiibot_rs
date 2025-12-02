use poise::serenity_prelude::{
    ChannelType, CreateActionRow, CreateButton, CreateMessage, GuildChannel,
};
use chrono::Timelike;
use tracing::{error, info, warn};

use crate::{
    models::{Context, Error},
    utils::timezone::{local_time_to_cron, parse_time_string},
    utils::messages::{build_delete_success, format_error, format_info},
    utils::channel_utils::format_birthday_setup_message,
    utils::validation::require_guild,
};

/// Setup birthday collection in a channel
#[poise::command(slash_command, required_permissions = "MANAGE_GUILD")]
pub async fn setup_birthday(
    ctx: Context<'_>,
    #[description = "Channel where birthday notifications will be sent"]
    notification_channel: GuildChannel,
    #[description = "Time to send birthday notifications (HH:MM, 24-hour format, default: 08:00)"]
    time: Option<String>,
    #[description = "Role to assign to users on their birthday (optional)"]
    birthday_role: Option<poise::serenity_prelude::Role>,
    #[description = "Custom message for users WITH age (use {user}, {date}, {mention}, {age})"]
    custom_message: Option<String>,
    #[description = "Custom message for users WITHOUT age (use {user}, {date}, {mention})"]
    custom_message_without_age: Option<String>,
    #[description = "Custom header message (shown once at the top)"]
    custom_header: Option<String>,
    #[description = "Custom footer message (shown once at the bottom)"]
    custom_footer: Option<String>,
    #[description = "Title for the birthday collection message"]
    collection_title: Option<String>,
    #[description = "Description for the birthday collection message"]
    collection_description: Option<String>,
    #[description = "Label for the button to set birthday"]
    collection_button: Option<String>,
) -> Result<(), Error> {
    let guild_id = require_guild(ctx.guild_id())?;

    // Verify it's a text channel
    if notification_channel.kind != ChannelType::Text {
        ctx.say(format_error("The notification channel must be a text channel!"))
            .await?;
        return Ok(());
    }

    // Parse the time (default to 08:00)
    let time_str = time.unwrap_or_else(|| "08:00".to_string());
    let parsed_time = match parse_time_string(&time_str) {
        Ok(t) => t,
        Err(e) => {
            ctx.say(format_error(&e.to_string())).await?;
            return Ok(());
        }
    };

    // Get the guild's timezone from database
    let tz_str = ctx
        .data()
        .db
        .get_guild_timezone(guild_id)
        .await
        .unwrap_or_else(|_| "UTC".to_string());

    // Convert local time to UTC cron expression
    let (cron_expr, utc_time) = match local_time_to_cron(&time_str, &tz_str) {
        Ok(result) => result,
        Err(e) => {
            ctx.say(format_error(&e.to_string())).await?;
            return Ok(());
        }
    };

    info!(
        "Timezone conversion: {} {} -> {} UTC (from timezone {})",
        parsed_time,
        tz_str,
        format!("{:02}:{:02}", utc_time.hour(), utc_time.minute()),
        tz_str
    );

    let birthday_role_id = birthday_role.as_ref().map(|r| r.id);

    // Save the birthday channel configuration
    if let Err(e) = ctx
        .data()
        .db
        .set_birthday_channel(
            guild_id,
            notification_channel.id,
            None,
            birthday_role_id,
            custom_message.clone(),
            custom_message_without_age.clone(),
            custom_header.clone(),
            custom_footer.clone(),
            collection_title.clone(),
            collection_description.clone(),
            collection_button.clone(),
        )
        .await
    {
        error!("Failed to save birthday channel to database: {}", e);
        ctx.say(format_error("Failed to save birthday channel configuration!"))
            .await?;
        return Ok(());
    }

    // Create or update the birthday schedule
    if let Err(e) = ctx
        .data()
        .db
        .upsert_schedule(
            Some(guild_id),
            crate::schedule::ScheduleType::Birthday,
            cron_expr,
            true,
        )
        .await
    {
        error!("Failed to save birthday schedule: {}", e);
        ctx.say(format_error("Failed to save birthday schedule!"))
            .await?;
        return Ok(());
    }

    // If a birthday role is specified, create/update the birthday role schedule at midnight
    if birthday_role_id.is_some() {
        let midnight_cron = match local_time_to_cron("00:00", &tz_str) {
            Ok((cron, _)) => cron,
            Err(e) => {
                warn!(
                    "Failed to create midnight cron for guild {}: {}",
                    guild_id, e
                );
                "0 0 0 * * *".to_string() // Fallback to UTC midnight
            }
        };

        if let Err(e) = ctx
            .data()
            .db
            .upsert_schedule(
                Some(guild_id),
                crate::schedule::ScheduleType::BirthdayRole,
                midnight_cron,
                true,
            )
            .await
        {
            error!("Failed to save birthday role schedule: {}", e);
            ctx.say(format_error("Failed to save birthday role schedule!"))
                .await?;
            return Ok(());
        }
    }

    // Signal schedule manager to reload
    let _ = ctx.data().schedule_reload_tx.send_modify(|val| *val += 1);
    info!("Triggered schedule reload after setup_birthday");

    // Create the birthday collection button
    let button_label = collection_button
        .as_deref()
        .unwrap_or("🎂 Set My Birthday")
        .replace("\\n", "\n");
    let button = CreateButton::new("collect_birthday")
        .label(button_label)
        .style(poise::serenity_prelude::ButtonStyle::Primary);

    let action_row = CreateActionRow::Buttons(vec![button]);

    // Build the collection message
    let title = collection_title
        .as_deref()
        .unwrap_or("🎉 **Birthday Collection** 🎉")
        .replace("\\n", "\n");
    let description = collection_description
        .as_deref()
        .unwrap_or(
            "Click the button below to set your birthday!\n\
            Your birthday will be celebrated across all servers where this bot is present.",
        )
        .replace("\\n", "\n")
        .to_string();

    let message_content = format!("{}\n\n{}\n", title, description);

    let message = CreateMessage::new()
        .content(message_content)
        .components(vec![action_row]);

    // Send the message in the current channel
    let sent_message = ctx.channel_id().send_message(ctx.http(), message).await?;

    // Update the database with the message ID
    if let Err(e) = ctx
        .data()
        .db
        .set_birthday_channel(
            guild_id,
            notification_channel.id,
            Some(sent_message.id),
            birthday_role_id,
            custom_message.clone(),
            custom_message_without_age.clone(),
            custom_header.clone(),
            custom_footer.clone(),
            collection_title.clone(),
            collection_description.clone(),
            collection_button.clone(),
        )
        .await
    {
        error!("Failed to update message_id in database: {}", e);
    }

    // Build response message using utility function
    let channel_mention = format!("<#{}>", notification_channel.id);
    let display_time = format!(
        "{} {} ({:02}:{:02} UTC)",
        time_str,
        tz_str,
        utc_time.hour(),
        utc_time.minute()
    );
    
    let base_message = format_birthday_setup_message(
        &channel_mention,
        &display_time,
        birthday_role.is_some(),
        &tz_str,
    );

    let custom_msg_info =
        if custom_message.is_some() || custom_message_without_age.is_some() || custom_header.is_some() || custom_footer.is_some() {
            let mut parts = vec![];
            if custom_message.is_some() {
                parts.push("with age");
            }
            if custom_message_without_age.is_some() {
                parts.push("without age");
            }
            if custom_header.is_some() || custom_footer.is_some() {
                parts.push("header/footer");
            }
            format!("\n\n📝 Custom messages configured ({})", parts.join(", "))
        } else {
            String::new()
        };

    let role_info = if let Some(role) = birthday_role {
        format!("\n🎭 Birthday role: <@&{}>", role.id)
    } else {
        String::new()
    };

    ctx.say(format!("{}{}{}", base_message, custom_msg_info, role_info))
        .await?;

    info!(
        "Setup birthday collection in guild {} with notification channel {} at {}",
        guild_id, notification_channel.id, time_str
    );

    Ok(())
}

/// Disable birthday notifications for this server
#[poise::command(slash_command, required_permissions = "MANAGE_GUILD")]
pub async fn disable_birthday(ctx: Context<'_>) -> Result<(), Error> {
    let guild_id = require_guild(ctx.guild_id())?;

    // Remove the birthday channel configuration for this guild
    match ctx.data().db.remove_birthday_channel(guild_id).await {
        Ok(Some((channel_id, message_id))) => {
            // Try to delete the message if we have a message_id
            if let Some(msg_id) = message_id {
                match channel_id.delete_message(ctx.http(), msg_id).await {
                    Ok(_) => {
                        info!(
                            "Deleted birthday collection message {} in channel {}",
                            msg_id, channel_id
                        );
                    }
                    Err(poise::serenity_prelude::Error::Http(http_error)) => {
                        // Check if it's just an "Unknown Message" error (message already deleted)
                        if http_error.to_string().contains("Unknown Message") {
                            info!(
                                "Birthday collection message {} was already deleted",
                                msg_id
                            );
                        } else {
                            warn!(
                                "Failed to delete birthday collection message {} in channel {}: {}",
                                msg_id, channel_id, http_error
                            );
                        }
                    }
                    Err(e) => {
                        warn!(
                            "Failed to delete birthday collection message {} in channel {}: {}",
                            msg_id, channel_id, e
                        );
                    }
                }
            }

            ctx.say(build_delete_success("Birthday notifications"))
                .await?;

            info!("Disabled birthday notifications for guild {}", guild_id);

            // Disable birthday schedule for this guild
            if let Err(e) = ctx
                .data()
                .db
                .set_schedule_enabled(
                    Some(guild_id),
                    crate::schedule::ScheduleType::Birthday,
                    false,
                )
                .await
            {
                error!("Failed to disable birthday schedule: {}", e);
            }

            // Signal schedule manager to reload
            let _ = ctx.data().schedule_reload_tx.send_modify(|val| *val += 1);
            info!("Triggered schedule reload after disable_birthday");

            // Note: We don't disable BirthdayRole schedule here because it's guild-specific
            // It will automatically be disabled when the birthday channel is removed
        }
        Ok(None) => {
            ctx.say(format_info("Birthday notifications were not configured for this server."))
                .await?;
        }
        Err(e) => {
            error!("Failed to remove birthday channel: {}", e);
            ctx.say(format_error("Failed to disable birthday notifications!"))
                .await?;
        }
    }

    Ok(())
}
