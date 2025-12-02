use poise::serenity_prelude::{self as serenity, ChannelId, GuildId, UserId, VoiceState};
use tracing::{error, info};

use crate::models::Data;

use super::channel::{create_temp_channel, delete_temp_channel, restore_archived_channel};

/// Handle voice state updates (user joins/leaves voice channels)
pub async fn handle_voice_state_update(
    ctx: &serenity::Context,
    old_state: Option<VoiceState>,
    new_state: VoiceState,
    data: &Data,
) {
    let guild_id = match new_state.guild_id {
        Some(id) => id,
        None => return,
    };

    // Handle user leaving a channel
    if let Some(old) = &old_state
        && let Some(old_channel_id) = old.channel_id
    {
        handle_user_left_channel(ctx, old_channel_id, data).await;
    }

    // Handle user joining a channel
    if let Some(new_channel_id) = new_state.channel_id {
        handle_user_joined_channel(ctx, new_channel_id, new_state.user_id, guild_id, data).await;
    }
}

/// Handle a user leaving a voice channel
async fn handle_user_left_channel(ctx: &serenity::Context, channel_id: ChannelId, data: &Data) {
    // Check if the user left a temporary channel
    let temp_channel_info = data.temp_channels.get(&channel_id).map(|tc| {
        (
            tc.owner_id,
            tc.is_persistent,
            tc.lobby_channel_id,
            tc.guild_id,
        )
    });

    if let Some((owner_id, is_persistent, lobby_channel_id, channel_guild_id)) = temp_channel_info {
        // Check if channel is empty
        if let Ok(channel) = channel_id.to_channel(ctx).await
            && let Some(guild_channel) = channel.guild()
            && let Ok(members) = guild_channel.members(ctx)
            && members.is_empty()
        {
            if is_persistent {
                // Archive the channel instead of deleting
                use super::channel::archive_channel;
                if let Err(e) =
                    archive_channel(ctx, channel_id, channel_guild_id, lobby_channel_id, data).await
                {
                    error!("Failed to archive channel: {}", e);
                } else {
                    info!(
                        "Archived persistent channel {} owned by {}",
                        channel_id, owner_id
                    );
                }
            } else {
                // Delete the empty temporary channel
                delete_temp_channel(ctx, channel_id, owner_id, data).await;
            }
        }
    }
}

/// Handle a user joining a voice channel
async fn handle_user_joined_channel(
    ctx: &serenity::Context,
    channel_id: ChannelId,
    user_id: UserId,
    guild_id: GuildId,
    data: &Data,
) {
    // Check if user joined a lobby channel
    if data.lobby_channels.contains_key(&channel_id) {
        let member = match guild_id.member(ctx, user_id).await {
            Ok(m) => m,
            Err(e) => {
                error!("Failed to get member: {}", e);
                return;
            }
        };

        // Check if user has an archived channel from this specific lobby to restore
        match data
            .db
            .get_archived_channel_for_user(guild_id, user_id, channel_id)
            .await
        {
            Ok(Some(archived_channel_id)) => {
                // Restore the archived channel
                match restore_archived_channel(ctx, &member, guild_id, archived_channel_id, data)
                    .await
                {
                    Ok(_) => {
                        // Successfully restored
                    }
                    Err(e) => {
                        error!("Failed to restore archived channel: {}", e);

                        // Clean up stale data - the channel likely doesn't exist anymore
                        data.temp_channels.remove(&archived_channel_id);
                        if let Err(db_err) = data.db.remove_temp_channel(archived_channel_id).await
                        {
                            error!(
                                "Failed to remove stale archived channel from database: {}",
                                db_err
                            );
                        } else {
                            info!(
                                "Cleaned up stale archived channel {} from database",
                                archived_channel_id
                            );
                        }

                        // Fall back to creating a new channel
                        if let Err(create_err) =
                            create_temp_channel(ctx, &member, guild_id, channel_id, data).await
                        {
                            error!("Failed to create temp channel: {}", create_err);
                        }
                    }
                }
            }
            Ok(None) => {
                // Create a new temporary channel for this user
                if let Err(e) = create_temp_channel(ctx, &member, guild_id, channel_id, data).await
                {
                    error!("Failed to create temp channel: {}", e);
                }
            }
            Err(e) => {
                error!("Failed to check for archived channel: {}", e);
                // Fall back to creating a new channel
                if let Err(e) = create_temp_channel(ctx, &member, guild_id, channel_id, data).await
                {
                    error!("Failed to create temp channel: {}", e);
                }
            }
        }
    }
}
