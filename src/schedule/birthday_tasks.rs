use chrono::{Datelike, Utc};
use poise::serenity_prelude::{self as serenity, ChannelId, CreateMessage, GuildId, UserId};
use std::sync::Arc;
use tracing::{error, info, warn};

use crate::models::Data;
use crate::utils::datetime::{format_date_display, get_current_month_day};
use crate::utils::message_formatter::{
    build_birthday_entry, build_combined_message, build_default_footer,
    build_default_header, format_age_info, join_birthday_entries, process_custom_text,
};
use crate::utils::role_logic::{determine_role_action, RoleAction};

/// Check for birthdays today and send notifications for a specific guild
pub async fn run_birthday_check(
    http: &Arc<serenity::Http>,
    _cache: &Arc<serenity::Cache>,
    data: &Data,
    guild_id: i64,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let (month, day) = get_current_month_day();
    
    let guild_id = serenity::GuildId::new(guild_id as u64);

    info!("Checking birthdays for {}/{} in guild {}", day, month, guild_id);

    // Get all users with birthdays today
    let birthdays = data.db.get_birthdays_on_date(month, day).await?;

    if birthdays.is_empty() {
        info!("No birthdays found for today");
        return Ok(());
    }

    info!("Found {} birthday(s) today", birthdays.len());

    // Get the birthday notification channel for this guild
    let channel_config = match data.db.get_birthday_channel(guild_id).await {
        Ok(Some(config)) => config,
        Ok(None) => {
            // No birthday channel configured for this guild
            info!("No birthday channel configured for guild {}", guild_id);
            return Ok(());
        }
        Err(e) => {
            error!(
                "Failed to get birthday channel for guild {}: {}",
                guild_id, e
            );
            return Err(Box::new(e));
        }
    };

    let (channel_id, _message_id, custom_message, custom_message_without_age, custom_header, custom_footer) = channel_config;

    // Filter birthdays to only include users who are in this guild (functional approach)
    let guild_birthdays: Vec<(UserId, Option<i32>)> = {
        let mut results = Vec::new();
        for (user_id, birth_year) in &birthdays {
            if guild_id.member(http, *user_id).await.is_ok() {
                results.push((*user_id, *birth_year));
            }
        }
        results
    };

    if guild_birthdays.is_empty() {
        info!("No birthday users are in guild {}", guild_id);
        return Ok(());
    }

    // Send a single combined birthday notification
    if let Err(e) = send_combined_birthday_notification(
        http,
        guild_id,
        channel_id,
        &guild_birthdays,
        &custom_message,
        &custom_message_without_age,
        &custom_header,
        &custom_footer,
    )
    .await
    {
        error!(
            "Failed to send birthday notification in guild {}: {}",
            guild_id, e
        );
    }

    Ok(())
}

/// Send a combined birthday notification for all users with birthdays today
async fn send_combined_birthday_notification(
    http: &Arc<serenity::Http>,
    guild_id: GuildId,
    channel_id: ChannelId,
    birthdays: &[(UserId, Option<i32>)],
    custom_message: &Option<String>,
    custom_message_without_age: &Option<String>,
    custom_header: &Option<String>,
    custom_footer: &Option<String>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let now = Utc::now();
    let date_str = format_date_display(now.month() as i32, now.day() as i32);
    let current_year = now.year();

    // Build the header using pure function
    let header = process_custom_text(custom_header)
        .unwrap_or_else(build_default_header);

    // Build the per-user messages using functional approach with pure functions
    let mut birthday_messages = Vec::new();
    for (user_id, birth_year) in birthdays {
        let user_name = guild_id
            .member(http, *user_id)
            .await
            .ok()
            .map(|m| m.display_name().to_string())
            .unwrap_or_else(|| "Unknown".to_string());

        let age_info = format_age_info(*birth_year, current_year);
        let mention = format!("<@{}>", user_id);

        let message = build_birthday_entry(
            &user_name,
            &mention,
            &age_info,
            custom_message, // template with age
            custom_message_without_age, // template without age
            &date_str,
        );
        
        birthday_messages.push(message);
    }
    
    let birthday_list = join_birthday_entries(&birthday_messages);

    // Build the footer using pure function
    let footer = process_custom_text(custom_footer)
        .unwrap_or_else(build_default_footer);

    // Combine everything using pure function
    let message_content = build_combined_message(&header, &birthday_list, &footer);

    // Send the message
    let message = CreateMessage::new().content(message_content);

    match channel_id.send_message(http, message).await {
        Ok(_) => {
            info!(
                "Sent birthday notification for {} user(s) in guild {}",
                birthdays.len(),
                guild_id
            );
        }
        Err(e) => {
            warn!(
                "Failed to send birthday message to channel {} in guild {}: {}",
                channel_id, guild_id, e
            );
            return Err(Box::new(e));
        }
    }

    Ok(())
}

/// Update birthday roles for all guilds
pub async fn run_birthday_role_update_all_guilds(
    http: &Arc<serenity::Http>,
    cache: &Arc<serenity::Cache>,
    data: &Data,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let (month, day) = get_current_month_day();

    info!("Updating birthday roles for {}/{} across all guilds", day, month);

    // Get all guilds the bot is in
    let guilds = cache.guilds();

    for guild_id in guilds {
        if let Err(e) = run_birthday_role_update(http, cache, data, guild_id.get() as i64).await {
            error!("Failed to update birthday roles for guild {}: {}", guild_id, e);
        }
    }

    Ok(())
}

/// Update birthday roles - assign to users with birthdays today, remove from others
pub async fn run_birthday_role_update(
    http: &Arc<serenity::Http>,
    _cache: &Arc<serenity::Cache>,
    data: &Data,
    guild_id: i64,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let (month, day) = get_current_month_day();
    
    let guild_id = serenity::GuildId::new(guild_id as u64);

    info!("Updating birthday roles for {}/{} in guild {}", day, month, guild_id);

    // Get all users with birthdays today
    let birthdays = data.db.get_birthdays_on_date(month, day).await?;
    let birthday_user_ids: std::collections::HashSet<UserId> = 
        birthdays.iter().map(|(user_id, _)| *user_id).collect();

    info!("Found {} user(s) with birthdays today", birthday_user_ids.len());

    // Get the birthday role configuration for this guild
    let role_id = match data.db.get_birthday_role(guild_id).await {
        Ok(Some(role)) => role,
        Ok(None) => {
            // No birthday role configured for this guild
            info!("No birthday role configured for guild {}", guild_id);
            return Ok(());
        }
        Err(e) => {
            error!(
                "Failed to get birthday role for guild {}: {}",
                guild_id, e
            );
            return Err(Box::new(e));
        }
    };

    // Get all members in the guild
    let members = match guild_id.members(http, None, None).await {
        Ok(m) => m,
        Err(e) => {
            error!("Failed to get members for guild {}: {}", guild_id, e);
            return Err(Box::new(e));
        }
    };

    // Process role updates using pure function
    for member in members {
        let has_birthday_today = birthday_user_ids.contains(&member.user.id);
        let has_birthday_role = member.roles.contains(&role_id);

        // Use pure function to determine action
        let action = determine_role_action(has_birthday_today, has_birthday_role);

        match action {
            RoleAction::Add => {
                // Add birthday role
                if let Err(e) = member.add_role(http, role_id).await {
                    error!(
                        "Failed to add birthday role to user {} in guild {}: {}",
                        member.user.id, guild_id, e
                    );
                } else {
                    info!(
                        "Added birthday role to user {} in guild {}",
                        member.user.id, guild_id
                    );
                }
            }
            RoleAction::Remove => {
                // Remove birthday role
                if let Err(e) = member.remove_role(http, role_id).await {
                    error!(
                        "Failed to remove birthday role from user {} in guild {}: {}",
                        member.user.id, guild_id, e
                    );
                } else {
                    info!(
                        "Removed birthday role from user {} in guild {}",
                        member.user.id, guild_id
                    );
                }
            }
            RoleAction::NoAction => {} // No action needed
        }
    }

    info!("Birthday role update completed");
    Ok(())
}
