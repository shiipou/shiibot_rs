use poise::serenity_prelude::{
    self as serenity, ChannelId, ChannelType, CreateActionRow, CreateButton, CreateChannel,
    CreateMessage, EditChannel, GetMessages, GuildId, Member, PermissionOverwrite,
    PermissionOverwriteType, Permissions, UserId,
};
use tracing::{error, info, warn};

use crate::{
    constants::{ARCHIVE_CATEGORY_NAME, MAX_MESSAGE_SCAN},
    models::{Data, Error, TempChannel},
    utils::channel_utils::format_temp_channel_name,
    utils::messages::build_context_error,
};

/// Create a temporary voice channel for a user
pub async fn create_temp_channel(
    ctx: &serenity::Context,
    member: &Member,
    guild_id: GuildId,
    lobby_channel_id: ChannelId,
    data: &Data,
) -> Result<(), Error> {
    let user_name = member.display_name();
    let channel_name = format_temp_channel_name(&user_name);

    // Get the lobby channel to copy its category and permissions
    let lobby_channel = lobby_channel_id.to_channel(ctx).await?;
    let guild_channel = lobby_channel
        .guild()
        .ok_or_else(|| build_context_error("as a guild channel"))?;
    let category_id = guild_channel.parent_id;

    // Get permission overwrites from the lobby channel
    let mut permissions: Vec<PermissionOverwrite> = guild_channel.permission_overwrites.to_vec();

    // Add permission overwrite for the owner (additional permissions on top of inherited ones)
    let owner_permissions = PermissionOverwrite {
        allow: Permissions::MANAGE_CHANNELS
            | Permissions::MOVE_MEMBERS
            | Permissions::MUTE_MEMBERS
            | Permissions::DEAFEN_MEMBERS,
        deny: Permissions::empty(),
        kind: PermissionOverwriteType::Member(member.user.id),
    };
    permissions.push(owner_permissions);

    // Build the channel creation request
    let mut create_channel = CreateChannel::new(&channel_name)
        .kind(ChannelType::Voice)
        .permissions(permissions);

    // Set category if lobby has one
    if let Some(cat_id) = category_id {
        create_channel = create_channel.category(cat_id);
    }

    // Create the temporary channel
    let temp_channel = guild_id.create_channel(ctx, create_channel).await?;

    // Store the temp channel in memory
    data.temp_channels.insert(
        temp_channel.id,
        TempChannel {
            owner_id: member.user.id,
            lobby_channel_id,
            is_persistent: false,
            is_archived: false,
            guild_id,
        },
    );

    // Save to database
    if let Err(e) = data
        .db
        .insert_temp_channel(temp_channel.id, guild_id, member.user.id, lobby_channel_id)
        .await
    {
        error!("Failed to save temp channel to database: {}", e);
    }

    // Move the user to their new channel
    guild_id
        .move_member(ctx, member.user.id, temp_channel.id)
        .await?;

    // Send configuration message
    send_channel_config_message(ctx, temp_channel.id, member, false).await?;

    info!(
        "Created temp channel {} for user {} in guild {}",
        temp_channel.id, member.user.id, guild_id
    );

    Ok(())
}

/// Delete a temporary channel and clean up
pub async fn delete_temp_channel(
    ctx: &serenity::Context,
    channel_id: ChannelId,
    owner_id: UserId,
    data: &Data,
) {
    if let Err(e) = channel_id.delete(ctx).await {
        error!("Failed to delete temp channel: {}", e);
    } else {
        data.temp_channels.remove(&channel_id);
        // Remove from database
        if let Err(e) = data.db.remove_temp_channel(channel_id).await {
            error!("Failed to remove temp channel from database: {}", e);
        }
        info!(
            "Deleted empty temp channel {} owned by {}",
            channel_id, owner_id
        );
    }
}

/// Send the configuration message with buttons in a voice channel
pub async fn send_channel_config_message(
    ctx: &serenity::Context,
    channel_id: ChannelId,
    member: &Member,
    is_persistent: bool,
) -> Result<(), Error> {
    let configure_button = CreateButton::new("configure_channel")
        .label("⚙️ Configure Channel")
        .style(serenity::ButtonStyle::Primary);

    let (persistent_label, persistent_style) = if is_persistent {
        ("📌 Remove Persistent", serenity::ButtonStyle::Danger)
    } else {
        ("📌 Make Persistent", serenity::ButtonStyle::Secondary)
    };

    let persistent_button = CreateButton::new("toggle_persistent")
        .label(persistent_label)
        .style(persistent_style);

    let action_row = CreateActionRow::Buttons(vec![configure_button, persistent_button]);

    let content = if is_persistent {
        format!(
            "🎙️ **Welcome back to your channel, {}!**\n\n\
            Your persistent channel has been restored from the archive.",
            member.display_name()
        )
    } else {
        format!(
            "🎙️ **Welcome to your temporary voice channel, {}!**\n\n\
            This channel will be automatically deleted when everyone leaves.\n\
            Click **Configure Channel** to rename it, or **Make Persistent** to keep it archived when empty.",
            member.display_name()
        )
    };

    let message = CreateMessage::new()
        .content(content)
        .components(vec![action_row]);

    channel_id.send_message(ctx, message).await?;

    Ok(())
}

/// Get or create an archive category for a guild
pub async fn get_or_create_archive_category(
    ctx: &serenity::Context,
    guild_id: GuildId,
    data: &Data,
) -> Result<ChannelId, Error> {
    // Check memory cache first
    if let Some(category_id) = data.archive_categories.get(&guild_id) {
        // Verify the category still exists
        if category_id.to_channel(ctx).await.is_ok() {
            return Ok(*category_id);
        }
        // Category was deleted, remove from cache
        data.archive_categories.remove(&guild_id);
    }

    // Check database
    if let Ok(Some(category_id)) = data.db.get_archive_category(guild_id).await {
        // Verify the category still exists
        if category_id.to_channel(ctx).await.is_ok() {
            data.archive_categories.insert(guild_id, category_id);
            return Ok(category_id);
        }
    }

    // Create new archive category with no permissions (invisible to everyone)
    let everyone_role = guild_id.everyone_role();
    let deny_permissions = PermissionOverwrite {
        allow: Permissions::empty(),
        deny: Permissions::VIEW_CHANNEL | Permissions::CONNECT,
        kind: PermissionOverwriteType::Role(everyone_role),
    };

    let category = guild_id
        .create_channel(
            ctx,
            CreateChannel::new(ARCHIVE_CATEGORY_NAME)
                .kind(ChannelType::Category)
                .permissions(vec![deny_permissions]),
        )
        .await?;

    // Save to database and cache
    if let Err(e) = data.db.set_archive_category(guild_id, category.id).await {
        error!("Failed to save archive category to database: {}", e);
    }
    data.archive_categories.insert(guild_id, category.id);

    info!(
        "Created archive category {} for guild {}",
        category.id, guild_id
    );

    Ok(category.id)
}

/// Archive a persistent channel by moving it to the archive category
pub async fn archive_channel(
    ctx: &serenity::Context,
    channel_id: ChannelId,
    guild_id: GuildId,
    _lobby_channel_id: ChannelId,
    data: &Data,
) -> Result<(), Error> {
    // Get or create the archive category
    let archive_category_id = get_or_create_archive_category(ctx, guild_id, data).await?;

    // Update the channel to be in the archive category with no visibility
    let everyone_role = guild_id.everyone_role();
    let deny_permissions = PermissionOverwrite {
        allow: Permissions::empty(),
        deny: Permissions::VIEW_CHANNEL | Permissions::CONNECT,
        kind: PermissionOverwriteType::Role(everyone_role),
    };

    channel_id
        .edit(
            ctx,
            EditChannel::new()
                .category(Some(archive_category_id))
                .permissions(vec![deny_permissions]),
        )
        .await?;

    // Update in memory
    if let Some(mut tc) = data.temp_channels.get_mut(&channel_id) {
        tc.is_archived = true;
    }

    // Update in database
    if let Err(e) = data.db.set_channel_archived(channel_id, true).await {
        error!(
            "Failed to update channel archived status in database: {}",
            e
        );
    }

    Ok(())
}

/// Restore an archived channel by moving it back and adding proper permissions
pub async fn restore_archived_channel(
    ctx: &serenity::Context,
    member: &Member,
    guild_id: GuildId,
    channel_id: ChannelId,
    data: &Data,
) -> Result<(), Error> {
    // Get the temp channel info
    let lobby_channel_id = {
        let tc = data
            .temp_channels
            .get(&channel_id)
            .ok_or_else(|| build_context_error("in temp channels"))?;
        tc.lobby_channel_id
    };

    // Get the lobby channel to copy its category and permissions
    let lobby_channel = lobby_channel_id.to_channel(ctx).await?;
    let guild_channel = lobby_channel
        .guild()
        .ok_or_else(|| build_context_error("as a guild channel"))?;
    let category_id = guild_channel.parent_id;

    // Get permission overwrites from the lobby channel
    let mut permissions: Vec<PermissionOverwrite> = guild_channel.permission_overwrites.to_vec();

    // Add permission overwrite for the owner
    let owner_permissions = PermissionOverwrite {
        allow: Permissions::MANAGE_CHANNELS
            | Permissions::MOVE_MEMBERS
            | Permissions::MUTE_MEMBERS
            | Permissions::DEAFEN_MEMBERS,
        deny: Permissions::empty(),
        kind: PermissionOverwriteType::Member(member.user.id),
    };
    permissions.push(owner_permissions);

    // Move channel back to lobby's category with proper permissions
    let mut edit = EditChannel::new().permissions(permissions);
    if let Some(cat_id) = category_id {
        edit = edit.category(Some(cat_id));
    }
    channel_id.edit(ctx, edit).await?;

    // Update in memory
    if let Some(mut tc) = data.temp_channels.get_mut(&channel_id) {
        tc.is_archived = false;
    }

    // Update in database
    if let Err(e) = data.db.set_channel_archived(channel_id, false).await {
        error!(
            "Failed to update channel archived status in database: {}",
            e
        );
    }

    // Move the user to their restored channel
    guild_id
        .move_member(ctx, member.user.id, channel_id)
        .await?;

    // Delete old bot messages that have buttons to keep chat clean
    clean_old_bot_messages(ctx, channel_id).await;

    // Send a welcome back message
    send_channel_config_message(ctx, channel_id, member, true).await?;

    info!(
        "Restored archived channel {} for user {} in guild {}",
        channel_id, member.user.id, guild_id
    );

    Ok(())
}

/// Clean up old bot messages with buttons from a channel
async fn clean_old_bot_messages(ctx: &serenity::Context, channel_id: ChannelId) {
    let bot_id = ctx.cache.current_user().id;
    if let Ok(messages) = channel_id
        .messages(ctx, GetMessages::new().limit(MAX_MESSAGE_SCAN))
        .await
    {
        for msg in messages {
            if msg.author.id == bot_id
                && !msg.components.is_empty()
                && let Err(e) = msg.delete(ctx).await
            {
                warn!("Failed to delete old bot message: {}", e);
            }
        }
    }
}
