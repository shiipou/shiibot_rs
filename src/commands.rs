use poise::serenity_prelude::{
    ChannelType, CreateActionRow, CreateButton, CreateChannel, CreateMessage, GuildChannel,
};
use chrono::{TimeZone, Timelike};
use tracing::{error, info, warn};

use crate::{
    constants::DEFAULT_LOBBY_NAME,
    models::{Context, Error},
};

/// Create a lobby voice channel that spawns temporary channels
#[poise::command(slash_command, required_permissions = "MANAGE_CHANNELS")]
pub async fn create_lobby(
    ctx: Context<'_>,
    #[description = "Name for the lobby channel"] name: Option<String>,
) -> Result<(), Error> {
    let guild_id = ctx
        .guild_id()
        .ok_or("This command must be used in a server")?;

    let lobby_name = name.unwrap_or_else(|| DEFAULT_LOBBY_NAME.to_string());

    // Create the lobby voice channel
    let channel = guild_id
        .create_channel(
            ctx.http(),
            CreateChannel::new(&lobby_name).kind(ChannelType::Voice),
        )
        .await?;

    // Store the lobby channel
    ctx.data().lobby_channels.insert(channel.id, guild_id);

    // Save to database
    if let Err(e) = ctx
        .data()
        .db
        .insert_lobby_channel(channel.id, guild_id)
        .await
    {
        error!("Failed to save lobby channel to database: {}", e);
    }

    ctx.say(format!(
        "✅ Created lobby channel: <#{}>. Users joining this channel will get their own temporary voice channel!",
        channel.id
    ))
    .await?;

    info!("Created lobby channel {} in guild {}", channel.id, guild_id);

    Ok(())
}

/// Convert an existing voice channel into a lobby managed by the bot
#[poise::command(slash_command, required_permissions = "MANAGE_CHANNELS")]
pub async fn convert_to_lobby(
    ctx: Context<'_>,
    #[description = "The voice channel to convert into a lobby"]
    #[channel_types("Voice")]
    channel: GuildChannel,
) -> Result<(), Error> {
    let guild_id = ctx
        .guild_id()
        .ok_or("This command must be used in a server")?;

    // Verify the channel is a voice channel
    if channel.kind != ChannelType::Voice {
        ctx.say("❌ The selected channel must be a voice channel!")
            .await?;
        return Ok(());
    }

    // Check if it's already a lobby
    if ctx.data().lobby_channels.contains_key(&channel.id) {
        ctx.say("❌ This channel is already a lobby!").await?;
        return Ok(());
    }

    // Check if it's a temp channel
    if ctx.data().temp_channels.contains_key(&channel.id) {
        ctx.say("❌ This channel is a temporary channel and cannot be converted to a lobby!")
            .await?;
        return Ok(());
    }

    // Store the lobby channel
    ctx.data().lobby_channels.insert(channel.id, guild_id);

    // Save to database
    if let Err(e) = ctx
        .data()
        .db
        .insert_lobby_channel(channel.id, guild_id)
        .await
    {
        error!("Failed to save lobby channel to database: {}", e);
    }

    ctx.say(format!(
        "✅ Converted <#{}> into a lobby channel! Users joining this channel will get their own temporary voice channel.",
        channel.id
    ))
    .await?;

    info!(
        "Converted channel {} to lobby in guild {}",
        channel.id, guild_id
    );

    Ok(())
}

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
    #[description = "Custom per-user message (use {user}, {date}, {mention})"]
    custom_message: Option<String>,
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
    let guild_id = ctx
        .guild_id()
        .ok_or("This command must be used in a server")?;

    // Verify it's a text channel
    if notification_channel.kind != ChannelType::Text {
        ctx.say("❌ The notification channel must be a text channel!")
            .await?;
        return Ok(());
    }

    // Parse the time (default to 08:00)
    let time_str = time.unwrap_or_else(|| "08:00".to_string());
    let parsed_time = match chrono::NaiveTime::parse_from_str(&time_str, "%H:%M") {
        Ok(t) => t,
        Err(_) => {
            ctx.say("❌ Invalid time format! Please use HH:MM (24-hour format), for example: 08:00 or 14:30")
                .await?;
            return Ok(());
        }
    };

    // Get the guild's timezone from database
    let tz_str = ctx.data().db.get_guild_timezone(guild_id).await.unwrap_or_else(|_| "UTC".to_string());
    let tz: chrono_tz::Tz = match tz_str.parse() {
        Ok(tz) => tz,
        Err(_) => {
            ctx.say(format!("❌ Invalid timezone stored in guild settings: '{}'. Please use /setup_timezone to set a valid timezone.", tz_str))
                .await?;
            return Ok(());
        }
    };

    // Convert local time to UTC for cron expression
    // We use a reference date to calculate the offset (today's date at the specified time)
    let today = chrono::Utc::now().date_naive();
    let local_datetime = today.and_time(parsed_time);
    
    // Handle potential DST ambiguity by using earliest() for consistency
    let local_datetime_tz = match tz.from_local_datetime(&local_datetime) {
        chrono::LocalResult::Single(dt) => dt,
        chrono::LocalResult::Ambiguous(dt1, _dt2) => dt1, // Use earliest during DST transition
        chrono::LocalResult::None => {
            ctx.say("❌ The specified time doesn't exist in this timezone (likely due to DST transition). Please choose a different time.")
                .await?;
            return Ok(());
        }
    };
    
    let utc_datetime = local_datetime_tz.with_timezone(&chrono::Utc);
    let utc_time = utc_datetime.time();

    info!(
        "Timezone conversion: {} {} -> {} UTC (from timezone {})",
        parsed_time, tz_str, 
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
            custom_header.clone(),
            custom_footer.clone(),
            collection_title.clone(),
            collection_description.clone(),
            collection_button.clone(),
        )
        .await
    {
        error!("Failed to save birthday channel to database: {}", e);
        ctx.say("❌ Failed to save birthday channel configuration!")
            .await?;
        return Ok(());
    }

    // Create or update the birthday schedule
    // Convert UTC time to cron expression with seconds (e.g., "0 0 8 * * *" for 8 AM UTC daily)
    // Format: second minute hour day month weekday
    let cron_expr = format!(
        "0 {} {} * * *",
        utc_time.minute(),
        utc_time.hour()
    );

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
        ctx.say("❌ Failed to save birthday schedule!")
            .await?;
        return Ok(());
    }

    // If a birthday role is specified, create/update the birthday role schedule at midnight
    // This schedule is guild-specific and runs at midnight in the guild's timezone
    if birthday_role_id.is_some() {
        // Calculate midnight in the guild's timezone, converted to UTC
        let midnight = chrono::NaiveTime::from_hms_opt(0, 0, 0).unwrap();
        let midnight_local = today.and_time(midnight);
        
        let midnight_tz = match tz.from_local_datetime(&midnight_local) {
            chrono::LocalResult::Single(dt) => dt,
            chrono::LocalResult::Ambiguous(dt1, _dt2) => dt1,
            chrono::LocalResult::None => {
                // Fallback to using the first valid time after the gap
                warn!("Midnight doesn't exist in timezone {} for guild {}, using fallback", tz_str, guild_id);
                tz.from_utc_datetime(&midnight_local)
            }
        };
        
        let utc_midnight = midnight_tz.with_timezone(&chrono::Utc);
        let utc_midnight_time = utc_midnight.time();
        
        // Cron expression for midnight in guild's timezone (sec min hour day month weekday)
        let midnight_cron = format!(
            "0 {} {} * * *",
            utc_midnight_time.minute(),
            utc_midnight_time.hour()
        );
        
        if let Err(e) = ctx
            .data()
            .db
            .upsert_schedule(
                Some(guild_id), // Guild-specific schedule
                crate::schedule::ScheduleType::BirthdayRole,
                midnight_cron,
                true,
            )
            .await
        {
            error!("Failed to save birthday role schedule: {}", e);
            ctx.say("❌ Failed to save birthday role schedule!")
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
        .replace("\\n", "\n"); // Convert literal \n to actual newlines
    let button = CreateButton::new("collect_birthday")
        .label(button_label)
        .style(poise::serenity_prelude::ButtonStyle::Primary);

    let action_row = CreateActionRow::Buttons(vec![button]);

    // Build the collection message
    let title = collection_title
        .as_deref()
        .unwrap_or("🎉 **Birthday Collection** 🎉")
        .replace("\\n", "\n"); // Convert literal \n to actual newlines
    let description = collection_description
        .as_deref()
        .unwrap_or(
            "Click the button below to set your birthday!\n\
            Your birthday will be celebrated across all servers where this bot is present."
        )
        .replace("\\n", "\n") // Convert literal \n to actual newlines
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

    let custom_msg_info = if custom_message.is_some() || custom_header.is_some() || custom_footer.is_some() {
        "\nCustom messages configured".to_string()
    } else {
        String::new()
    };

    let role_info = if let Some(role) = birthday_role {
        format!("\nBirthday role: <@&{}>", role.id)
    } else {
        String::new()
    };

    ctx.say(format!(
        "✅ Birthday system configured!\n\
        Notifications will be sent to <#{}> at {} {} (stored as {} UTC){}{}",
        notification_channel.id, time_str, tz_str, 
        format!("{:02}:{:02}", utc_time.hour(), utc_time.minute()),
        custom_msg_info, role_info
    ))
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
    let guild_id = ctx
        .guild_id()
        .ok_or("This command must be used in a server")?;

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
                    Err(serenity::Error::Http(http_error)) => {
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

            ctx.say("✅ Birthday notifications have been disabled for this server!")
                .await?;
            
            info!("Disabled birthday notifications for guild {}", guild_id);

            // Disable birthday schedule for this guild
            if let Err(e) = ctx
                .data()
                .db
                .set_schedule_enabled(Some(guild_id), crate::schedule::ScheduleType::Birthday, false)
                .await
            {
                error!("Failed to disable birthday schedule: {}", e);
            }
            
            // Signal schedule manager to reload
            let _ = ctx.data().schedule_reload_tx.send_modify(|val| *val += 1);
            info!("Triggered schedule reload after disable_birthday");
            
            // Note: We don't disable BirthdayRole schedule here because it's global
            // It will automatically skip guilds without a birthday role configured
        }
        Ok(None) => {
            ctx.say("ℹ️ Birthday notifications were not configured for this server.")
                .await?;
        }
        Err(e) => {
            error!("Failed to remove birthday channel: {}", e);
            ctx.say("❌ Failed to disable birthday notifications!")
                .await?;
        }
    }

    Ok(())
}