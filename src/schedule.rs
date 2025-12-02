use chrono::{Datelike, Utc};
use poise::serenity_prelude::{self as serenity, ChannelId, CreateMessage, GuildId, UserId};
use std::str::FromStr;
use std::sync::Arc;
use tokio::time::{Duration, sleep};
use tracing::{error, info, warn};

use crate::models::Data;

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

/// Start the schedule manager that monitors and runs scheduled tasks
pub fn start_schedule_manager(
    ctx: Arc<serenity::Http>,
    cache: Arc<serenity::Cache>,
    data: Arc<Data>,
) {
    tokio::spawn(async move {
        info!("Schedule manager started");
        
        let mut reload_rx = data.schedule_reload_tx.subscribe();

        loop {
            // Load schedules from database
            match data.db.get_all_schedules().await {
                Ok(schedules) => {
                    if schedules.is_empty() {
                        info!("No schedules configured, waiting for schedules to be added...");
                        // Wait for a reload signal instead of sleeping for an hour
                        if reload_rx.changed().await.is_ok() {
                            info!("Schedule reload signal received, reloading schedules");
                            continue;
                        } else {
                            // Channel closed, exit
                            break;
                        }
                    }

                    // Find the next schedule to run
                    if let Some((schedule, wait_duration)) = find_next_schedule(&schedules) {
                        info!(
                            "Next {:?} schedule (cron: '{}') will run in {} minutes",
                            schedule.schedule_type,
                            schedule.cron_expression,
                            wait_duration.as_secs() / 60
                        );

                        // Wait until it's time to run OR until we get a reload signal
                        tokio::select! {
                            _ = sleep(wait_duration) => {
                                // Time to run the scheduled task
                                if let Err(e) = run_schedule(&ctx, &cache, &data, &schedule).await {
                                    error!("Failed to run {:?} schedule: {}", schedule.schedule_type, e);
                                }
                            }
                            _ = reload_rx.changed() => {
                                // Reload signal received, restart the loop
                                info!("Schedule reload signal received, reconfiguring schedules");
                                continue;
                            }
                        }
                    } else {
                        // No valid schedules, wait for a reload signal
                        info!("No valid schedules found, waiting for configuration...");
                        if reload_rx.changed().await.is_ok() {
                            info!("Schedule reload signal received, reloading schedules");
                            continue;
                        } else {
                            // Channel closed, exit
                            break;
                        }
                    }
                }
                Err(e) => {
                    error!("Failed to load schedules from database: {}", e);
                    sleep(Duration::from_secs(60)).await; // Retry in 1 minute
                }
            }
        }
        
        info!("Schedule manager stopped");
    });
}

/// Find the next schedule to run and calculate wait duration
fn find_next_schedule(schedules: &[Schedule]) -> Option<(Schedule, Duration)> {
    let now = Utc::now();
    let mut next_schedule: Option<(Schedule, Duration)> = None;

    for schedule in schedules {
        if !schedule.enabled {
            continue;
        }

        // Parse cron expression
        let cron_schedule = match cron::Schedule::from_str(&schedule.cron_expression) {
            Ok(s) => s,
            Err(e) => {
                error!(
                    "Invalid cron expression '{}' for {:?} schedule: {}",
                    schedule.cron_expression, schedule.schedule_type, e
                );
                continue;
            }
        };

        // Find next occurrence
        let next_time = match cron_schedule.upcoming(Utc).next() {
            Some(t) => t,
            None => {
                warn!(
                    "No upcoming time found for {:?} schedule with cron '{}'",
                    schedule.schedule_type, schedule.cron_expression
                );
                continue;
            }
        };

        let wait_duration = (next_time - now)
            .to_std()
            .unwrap_or(Duration::from_secs(60));

        // Keep track of the soonest schedule
        if let Some((_, current_wait)) = &next_schedule {
            if wait_duration < *current_wait {
                next_schedule = Some((schedule.clone(), wait_duration));
            }
        } else {
            next_schedule = Some((schedule.clone(), wait_duration));
        }
    }

    if let Some((schedule, duration)) = &next_schedule {
        info!(
            "Next {:?} schedule with cron '{}' in {} minutes",
            schedule.schedule_type,
            schedule.cron_expression,
            duration.as_secs() / 60
        );
    }

    next_schedule
}

/// Run a scheduled task based on its type
async fn run_schedule(
    http: &Arc<serenity::Http>,
    cache: &Arc<serenity::Cache>,
    data: &Data,
    schedule: &Schedule,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    match schedule.schedule_type {
        ScheduleType::Birthday => {
            // Birthday notifications are guild-specific
            if let Some(guild_id) = schedule.guild_id {
                run_birthday_check(http, cache, data, guild_id).await
            } else {
                error!("Birthday schedule has no guild_id, skipping");
                Ok(())
            }
        }
        ScheduleType::BirthdayRole => {
            // BirthdayRole can be guild-specific or global
            if let Some(guild_id) = schedule.guild_id {
                // Guild-specific: run for this guild only
                run_birthday_role_update(http, cache, data, guild_id).await
            } else {
                // Global: run for all guilds (legacy behavior)
                run_birthday_role_update_all_guilds(http, cache, data).await
            }
        }
    }
}

/// Check for birthdays today and send notifications for a specific guild
async fn run_birthday_check(
    http: &Arc<serenity::Http>,
    _cache: &Arc<serenity::Cache>,
    data: &Data,
    guild_id: i64,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let now = Utc::now();
    let month = now.month() as i32;
    let day = now.day() as i32;
    
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

    let (channel_id, _message_id, custom_message, custom_header, custom_footer) = channel_config;

    // Filter birthdays to only include users who are in this guild
    let mut guild_birthdays = Vec::new();
    for (user_id, birth_year) in &birthdays {
        // Check if user is in the guild
        if guild_id.member(http, *user_id).await.is_ok() {
            guild_birthdays.push((*user_id, *birth_year));
        }
    }

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
    custom_header: &Option<String>,
    custom_footer: &Option<String>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let now = Utc::now();
    let month_name = get_month_name(now.month() as i32);
    let date_str = format!("{} {}", now.day(), month_name);

    // Build the header (convert literal \n to actual newlines)
    let header = if let Some(h) = custom_header {
        h.replace("\\n", "\n")
    } else {
        "🎉 **Happy Birthday** 🎉\n\nToday we celebrate:".to_string()
    };

    // Build the per-user messages
    let mut birthday_list = String::new();
    for (user_id, birth_year) in birthdays {
        // Get user's display name
        let user_name = match guild_id.member(http, *user_id).await {
            Ok(member) => member.display_name().to_string(),
            Err(_) => "Unknown".to_string(),
        };

        // Calculate age if birth year is provided
        let age_info = if let Some(year) = birth_year {
            let current_year = Utc::now().year();
            let age = current_year - year;
            format!(" (turning {})", age)
        } else {
            String::new()
        };

        if let Some(template) = custom_message {
            // Use custom per-user message (convert literal \n to actual newlines)
            let msg = template
                .replace("{user}", &user_name)
                .replace("{date}", &date_str)
                .replace("{mention}", &format!("<@{}>", user_id))
                .replace("{age}", &age_info.trim_start_matches(" (turning ").trim_end_matches(")"))
                .replace("\\n", "\n");
            birthday_list.push_str(&format!("{}\n", msg));
        } else {
            // Default per-user format
            birthday_list.push_str(&format!("• <@{}>{}!\n", user_id, age_info));
        }
    }

    // Build the footer (convert literal \n to actual newlines)
    let footer = if let Some(f) = custom_footer {
        f.replace("\\n", "\n")
    } else {
        "\nEveryone wish them a happy birthday! 🎂🎈".to_string()
    };

    // Combine everything
    let message_content = format!(
        "{}\n{}\n{}",
        header,
        birthday_list.trim_end(),
        footer
    );

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

/// Get month name from month number
fn get_month_name(month: i32) -> &'static str {
    match month {
        1 => "January",
        2 => "February",
        3 => "March",
        4 => "April",
        5 => "May",
        6 => "June",
        7 => "July",
        8 => "August",
        9 => "September",
        10 => "October",
        11 => "November",
        12 => "December",
        _ => "Unknown",
    }
}

/// Update birthday roles for all guilds
async fn run_birthday_role_update_all_guilds(
    http: &Arc<serenity::Http>,
    cache: &Arc<serenity::Cache>,
    data: &Data,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let now = Utc::now();
    let month = now.month() as i32;
    let day = now.day() as i32;

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
async fn run_birthday_role_update(
    http: &Arc<serenity::Http>,
    _cache: &Arc<serenity::Cache>,
    data: &Data,
    guild_id: i64,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let now = Utc::now();
    let month = now.month() as i32;
    let day = now.day() as i32;
    
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

    for member in members {
        let has_birthday_today = birthday_user_ids.contains(&member.user.id);
        let has_birthday_role = member.roles.contains(&role_id);

        if has_birthday_today && !has_birthday_role {
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
        } else if !has_birthday_today && has_birthday_role {
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
    }

    info!("Birthday role update completed");
    Ok(())
}
