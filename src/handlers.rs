use poise::serenity_prelude::{
    self as serenity, ChannelId, ChannelType, CreateActionRow, CreateButton, CreateChannel,
    CreateInteractionResponse, CreateInteractionResponseMessage, CreateMessage, EditChannel,
    EditInteractionResponse, GetMessages, GuildId, Member, PermissionOverwrite,
    PermissionOverwriteType, Permissions, UserId, VoiceState,
};
use tracing::{error, info, warn};

use crate::{
    constants::{ARCHIVE_CATEGORY_NAME, MAX_CHANNEL_NAME_LENGTH, MAX_MESSAGE_SCAN},
    models::{Data, Error, TempChannel},
};

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

/// Delete a temporary channel and clean up
async fn delete_temp_channel(
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

/// Create a temporary voice channel for a user
async fn create_temp_channel(
    ctx: &serenity::Context,
    member: &Member,
    guild_id: GuildId,
    lobby_channel_id: ChannelId,
    data: &Data,
) -> Result<(), Error> {
    let user_name = member.display_name();
    let channel_name = format!("{}'s Channel", user_name);

    // Get the lobby channel to copy its category and permissions
    let lobby_channel = lobby_channel_id.to_channel(ctx).await?;
    let guild_channel = lobby_channel
        .guild()
        .ok_or("Lobby channel is not a guild channel")?;
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

/// Send the configuration message with buttons in a voice channel
async fn send_channel_config_message(
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
async fn get_or_create_archive_category(
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
async fn archive_channel(
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
async fn restore_archived_channel(
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
            .ok_or("Channel not found in temp channels")?;
        tc.lobby_channel_id
    };

    // Get the lobby channel to copy its category and permissions
    let lobby_channel = lobby_channel_id.to_channel(ctx).await?;
    let guild_channel = lobby_channel
        .guild()
        .ok_or("Lobby channel is not a guild channel")?;
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

/// Handle component interactions (button clicks)
pub async fn handle_interaction(
    ctx: &serenity::Context,
    interaction: serenity::ComponentInteraction,
    data: &Data,
) {
    match interaction.data.custom_id.as_str() {
        "configure_channel" => {
            if let Err(e) = handle_configure_button(ctx, &interaction, data).await {
                error!("Failed to handle configure button: {}", e);
            }
        }
        "toggle_persistent" => {
            if let Err(e) = handle_toggle_persistent_button(ctx, &interaction, data).await {
                error!("Failed to handle toggle persistent button: {}", e);
            }
        }
        _ => {}
    }
}

/// Handle the configure channel button
async fn handle_configure_button(
    ctx: &serenity::Context,
    interaction: &serenity::ComponentInteraction,
    data: &Data,
) -> Result<(), Error> {
    let channel_id = interaction.channel_id;
    let user_id = interaction.user.id;

    // Check if this is a temp channel and the user is the owner
    if !data.is_channel_owner(channel_id, user_id) {
        let response = CreateInteractionResponse::Message(
            CreateInteractionResponseMessage::new()
                .content("❌ Only the channel owner can configure this channel!")
                .ephemeral(true),
        );
        interaction.create_response(ctx, response).await?;
        return Ok(());
    }

    // Show modal for channel configuration
    let modal = serenity::CreateModal::new("channel_config_modal", "Configure Your Channel")
        .components(vec![serenity::CreateActionRow::InputText(
            serenity::CreateInputText::new(
                serenity::InputTextStyle::Short,
                "Channel Name",
                "channel_name",
            )
            .placeholder("Enter a new name for your channel")
            .required(true)
            .max_length(MAX_CHANNEL_NAME_LENGTH),
        )]);

    let response = CreateInteractionResponse::Modal(modal);
    interaction.create_response(ctx, response).await?;

    Ok(())
}

/// Handle the toggle persistent button
async fn handle_toggle_persistent_button(
    ctx: &serenity::Context,
    interaction: &serenity::ComponentInteraction,
    data: &Data,
) -> Result<(), Error> {
    let channel_id = interaction.channel_id;
    let user_id = interaction.user.id;

    // Check if this is a temp channel and the user is the owner
    if !data.is_channel_owner(channel_id, user_id) {
        let response = CreateInteractionResponse::Message(
            CreateInteractionResponseMessage::new()
                .content("❌ Only the channel owner can change persistence settings!")
                .ephemeral(true),
        );
        interaction.create_response(ctx, response).await?;
        return Ok(());
    }

    // Get current persistence state and lobby channel id
    let (is_currently_persistent, current_lobby_id) = data
        .temp_channels
        .get(&channel_id)
        .map(|tc| (tc.is_persistent, tc.lobby_channel_id))
        .ok_or("Channel not found in temp channels")?;

    let new_persistent_state = !is_currently_persistent;

    // Check if user already has another persistent channel from the same lobby (only when enabling)
    if new_persistent_state {
        let guild_id = interaction
            .guild_id
            .ok_or("This command must be used in a server")?;

        // Check if user has another persistent channel from the same lobby
        // Also verify the channel actually exists on Discord
        let mut stale_channels = Vec::new();
        let mut has_valid_persistent = false;

        for entry in data.temp_channels.iter() {
            let tc = entry.value();
            let other_channel_id = *entry.key();

            if tc.owner_id == user_id
                && tc.guild_id == guild_id
                && tc.lobby_channel_id == current_lobby_id
                && tc.is_persistent
                && other_channel_id != channel_id
            {
                // Verify the channel still exists on Discord
                match other_channel_id.to_channel(ctx).await {
                    Ok(_) => {
                        has_valid_persistent = true;
                        break;
                    }
                    Err(_) => {
                        // Channel doesn't exist anymore, mark for cleanup
                        stale_channels.push(other_channel_id);
                    }
                }
            }
        }

        // Clean up stale channels from memory and database
        for stale_channel_id in stale_channels {
            data.temp_channels.remove(&stale_channel_id);
            if let Err(e) = data.db.remove_temp_channel(stale_channel_id).await {
                error!(
                    "Failed to remove stale channel {} from database: {}",
                    stale_channel_id, e
                );
            } else {
                info!(
                    "Cleaned up stale channel {} from database",
                    stale_channel_id
                );
            }
        }

        if has_valid_persistent {
            let response = CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new()
                    .content(
                        "❌ You already have a persistent channel from this lobby! \
                        Please disable persistence on your other channel first.",
                    )
                    .ephemeral(true),
            );
            interaction.create_response(ctx, response).await?;
            return Ok(());
        }
    }

    // Update in memory
    if let Some(mut tc) = data.temp_channels.get_mut(&channel_id) {
        tc.is_persistent = new_persistent_state;
    }

    // Update in database
    if let Err(e) = data
        .db
        .set_channel_persistent(channel_id, new_persistent_state)
        .await
    {
        error!("Failed to update channel persistence in database: {}", e);
    }

    // Send response
    let (message, button_label, button_style) = if new_persistent_state {
        (
            "✅ **Channel is now persistent!**\n\n\
            When everyone leaves, this channel will be archived instead of deleted.\n\
            When you join the lobby again, your channel will be restored.",
            "📌 Remove Persistent",
            serenity::ButtonStyle::Danger,
        )
    } else {
        (
            "✅ **Channel is no longer persistent.**\n\n\
            When everyone leaves, this channel will be deleted.",
            "📌 Make Persistent",
            serenity::ButtonStyle::Secondary,
        )
    };

    // Update the message with new button state
    let configure_button = CreateButton::new("configure_channel")
        .label("⚙️ Configure Channel")
        .style(serenity::ButtonStyle::Primary);

    let persistent_button = CreateButton::new("toggle_persistent")
        .label(button_label)
        .style(button_style);

    let action_row = CreateActionRow::Buttons(vec![configure_button, persistent_button]);

    let response = CreateInteractionResponse::UpdateMessage(
        CreateInteractionResponseMessage::new()
            .content(message)
            .components(vec![action_row]),
    );
    interaction.create_response(ctx, response).await?;

    info!(
        "User {} set channel {} persistence to {}",
        user_id, channel_id, new_persistent_state
    );

    Ok(())
}

/// Handle modal submissions
pub async fn handle_modal_submit(
    ctx: &serenity::Context,
    interaction: serenity::ModalInteraction,
    data: &Data,
) {
    if interaction.data.custom_id == "channel_config_modal"
        && let Err(e) = handle_channel_config_modal(ctx, &interaction, data).await
    {
        error!("Failed to handle modal submission: {}", e);
    }
}

/// Handle the channel configuration modal submission
async fn handle_channel_config_modal(
    ctx: &serenity::Context,
    interaction: &serenity::ModalInteraction,
    data: &Data,
) -> Result<(), Error> {
    let channel_id = interaction.channel_id;
    let user_id = interaction.user.id;

    // Verify ownership
    if !data.is_channel_owner(channel_id, user_id) {
        let response = CreateInteractionResponse::Message(
            CreateInteractionResponseMessage::new()
                .content("❌ Only the channel owner can configure this channel!")
                .ephemeral(true),
        );
        interaction.create_response(ctx, response).await?;
        return Ok(());
    }

    // Get the new channel name from the modal
    let new_name = interaction
        .data
        .components
        .first()
        .and_then(|row| row.components.first())
        .and_then(|component| match component {
            serenity::ActionRowComponent::InputText(input) => input.value.clone(),
            _ => None,
        })
        .unwrap_or_default();

    if new_name.is_empty() {
        let response = CreateInteractionResponse::Message(
            CreateInteractionResponseMessage::new()
                .content("❌ Channel name cannot be empty!")
                .ephemeral(true),
        );
        interaction.create_response(ctx, response).await?;
        return Ok(());
    }

    // Defer the response first
    interaction
        .create_response(
            ctx,
            CreateInteractionResponse::Defer(
                CreateInteractionResponseMessage::new().ephemeral(true),
            ),
        )
        .await?;

    // Update the channel name
    channel_id
        .edit(ctx, EditChannel::new().name(&new_name))
        .await?;

    // Send follow-up response
    interaction
        .edit_response(
            ctx,
            EditInteractionResponse::new()
                .content(format!("✅ Channel renamed to **{}**!", new_name)),
        )
        .await?;

    info!(
        "User {} renamed temp channel {} to '{}'",
        user_id, channel_id, new_name
    );

    Ok(())
}
