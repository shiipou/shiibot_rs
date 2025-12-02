use chrono::Utc;
use poise::serenity_prelude as serenity;
use std::str::FromStr;
use std::sync::Arc;
use tokio::time::{Duration, sleep};
use tracing::{error, info, warn};

use crate::models::Data;
use super::{Schedule, ScheduleType};
use super::birthday_tasks::{run_birthday_check, run_birthday_role_update, run_birthday_role_update_all_guilds};

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

/// Find the next schedule to run and calculate wait duration (more functional approach)
fn find_next_schedule(schedules: &[Schedule]) -> Option<(Schedule, Duration)> {
    let now = Utc::now();
    
    schedules
        .iter()
        .filter(|s| s.enabled)
        .filter_map(|schedule| {
            // Parse cron expression
            let cron_schedule = cron::Schedule::from_str(&schedule.cron_expression)
                .map_err(|e| {
                    error!(
                        "Invalid cron expression '{}' for {:?} schedule: {}",
                        schedule.cron_expression, schedule.schedule_type, e
                    );
                    e
                })
                .ok()?;

            // Find next occurrence
            let next_time = cron_schedule.upcoming(Utc).next()
                .or_else(|| {
                    warn!(
                        "No upcoming time found for {:?} schedule with cron '{}'",
                        schedule.schedule_type, schedule.cron_expression
                    );
                    None
                })?;

            let wait_duration = (next_time - now)
                .to_std()
                .unwrap_or(Duration::from_secs(60));

            Some((schedule.clone(), wait_duration))
        })
        .min_by_key(|(_, duration)| *duration)
        .map(|(schedule, duration)| {
            info!(
                "Next {:?} schedule with cron '{}' in {} minutes",
                schedule.schedule_type,
                schedule.cron_expression,
                duration.as_secs() / 60
            );
            (schedule, duration)
        })
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
