mod db;

use dashmap::DashMap;
use db::Database;
use poise::serenity_prelude::{
    self as serenity, ChannelId, ChannelType, CreateActionRow, CreateButton, CreateChannel,
    CreateInteractionResponse, CreateInteractionResponseMessage, CreateMessage, EditChannel,
    EditInteractionResponse, GetMessages, GuildChannel, GuildId, Member, PermissionOverwrite,
    PermissionOverwriteType, Permissions, UserId, VoiceState,
};
use tracing::{error, info, warn};

/// Represents a temporary voice channel owned by a user
#[derive(Clone)]
struct TempChannel {
    owner_id: UserId,
    lobby_channel_id: ChannelId,
    is_persistent: bool,
    is_archived: bool,
    guild_id: GuildId,
}

/// Bot state shared across all handlers
struct Data {
    /// Database connection
    db: Database,
    /// Maps lobby channel IDs to guild IDs
    lobby_channels: DashMap<ChannelId, GuildId>,
    /// Maps temporary channel IDs to their data
    temp_channels: DashMap<ChannelId, TempChannel>,
    /// Maps guild IDs to their archive category IDs
    archive_categories: DashMap<GuildId, ChannelId>,
}

type Error = Box<dyn std::error::Error + Send + Sync>;
type Context<'a> = poise::Context<'a, Data, Error>;

/// Create a lobby voice channel that spawns temporary channels
#[poise::command(slash_command, required_permissions = "MANAGE_CHANNELS")]
async fn create_lobby(
    ctx: Context<'_>,
    #[description = "Name for the lobby channel"] name: Option<String>,
) -> Result<(), Error> {
    let guild_id = ctx
        .guild_id()
        .ok_or("This command must be used in a server")?;
    let lobby_name = name.unwrap_or_else(|| "➕ Create Voice Channel".to_string());

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
async fn convert_to_lobby(
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

/// Handle voice state updates (user joins/leaves voice channels)
async fn handle_voice_state_update(
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
    if let Some(old) = &old_state {
        if let Some(old_channel_id) = old.channel_id {
            // Check if the user left a temporary channel
            // Get channel info first and drop the guard to avoid deadlock
            let temp_channel_info = data.temp_channels.get(&old_channel_id).map(|tc| {
                (
                    tc.owner_id,
                    tc.is_persistent,
                    tc.lobby_channel_id,
                    tc.guild_id,
                )
            });

            if let Some((owner_id, is_persistent, lobby_channel_id, channel_guild_id)) =
                temp_channel_info
            {
                // Check if channel is empty
                if let Ok(channel) = old_channel_id.to_channel(ctx).await {
                    if let Some(guild_channel) = channel.guild() {
                        if let Ok(members) = guild_channel.members(ctx) {
                            if members.is_empty() {
                                if is_persistent {
                                    // Archive the channel instead of deleting
                                    if let Err(e) = archive_channel(
                                        ctx,
                                        old_channel_id,
                                        channel_guild_id,
                                        lobby_channel_id,
                                        data,
                                    )
                                    .await
                                    {
                                        error!("Failed to archive channel: {}", e);
                                    } else {
                                        info!(
                                            "Archived persistent channel {} owned by {}",
                                            old_channel_id, owner_id
                                        );
                                    }
                                } else {
                                    // Delete the empty temporary channel
                                    if let Err(e) = old_channel_id.delete(ctx).await {
                                        error!("Failed to delete temp channel: {}", e);
                                    } else {
                                        data.temp_channels.remove(&old_channel_id);
                                        // Remove from database
                                        if let Err(e) =
                                            data.db.remove_temp_channel(old_channel_id).await
                                        {
                                            error!(
                                                "Failed to remove temp channel from database: {}",
                                                e
                                            );
                                        }
                                        info!(
                                            "Deleted empty temp channel {} owned by {}",
                                            old_channel_id, owner_id
                                        );
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // Handle user joining a channel
    if let Some(new_channel_id) = new_state.channel_id {
        // Check if user joined a lobby channel
        if data.lobby_channels.contains_key(&new_channel_id) {
            let user_id = new_state.user_id;
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
                .get_archived_channel_for_user(guild_id, user_id, new_channel_id)
                .await
            {
                Ok(Some(archived_channel_id)) => {
                    // Restore the archived channel
                    if let Err(e) =
                        restore_archived_channel(ctx, &member, guild_id, archived_channel_id, data)
                            .await
                    {
                        error!("Failed to restore archived channel: {}", e);
                        // Fall back to creating a new channel
                        if let Err(e) =
                            create_temp_channel(ctx, &member, guild_id, new_channel_id, data).await
                        {
                            error!("Failed to create temp channel: {}", e);
                        }
                    }
                }
                Ok(None) => {
                    // Create a new temporary channel for this user
                    if let Err(e) =
                        create_temp_channel(ctx, &member, guild_id, new_channel_id, data).await
                    {
                        error!("Failed to create temp channel: {}", e);
                    }
                }
                Err(e) => {
                    error!("Failed to check for archived channel: {}", e);
                    // Fall back to creating a new channel
                    if let Err(e) =
                        create_temp_channel(ctx, &member, guild_id, new_channel_id, data).await
                    {
                        error!("Failed to create temp channel: {}", e);
                    }
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

    // Send configuration message in the voice channel's text chat
    let configure_button = CreateButton::new("configure_channel")
        .label("⚙️ Configure Channel")
        .style(serenity::ButtonStyle::Primary);

    let persistent_button = CreateButton::new("toggle_persistent")
        .label("📌 Make Persistent")
        .style(serenity::ButtonStyle::Secondary);

    let action_row = CreateActionRow::Buttons(vec![configure_button, persistent_button]);

    let message = CreateMessage::new()
        .content(format!(
            "🎙️ **Welcome to your temporary voice channel, {}!**\n\n\
            This channel will be automatically deleted when everyone leaves.\n\
            Click **Configure Channel** to rename it, or **Make Persistent** to keep it archived when empty.",
            member.display_name()
        ))
        .components(vec![action_row]);

    temp_channel.send_message(ctx, message).await?;

    info!(
        "Created temp channel {} for user {} in guild {}",
        temp_channel.id, member.user.id, guild_id
    );

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
            CreateChannel::new("📦 Archived Channels")
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
    lobby_channel_id: ChannelId,
    data: &Data,
) -> Result<(), Error> {
    // Get or create the archive category
    let archive_category_id = get_or_create_archive_category(ctx, guild_id, data).await?;

    // Get the lobby channel permissions to restore later
    let lobby_channel = lobby_channel_id.to_channel(ctx).await?;
    let _lobby_permissions = lobby_channel
        .guild()
        .map(|c| c.permission_overwrites.to_vec())
        .unwrap_or_default();

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
    let (lobby_channel_id, _) = {
        let tc = data
            .temp_channels
            .get(&channel_id)
            .ok_or("Channel not found in temp channels")?;
        (tc.lobby_channel_id, tc.owner_id)
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
    let bot_id = ctx.cache.current_user().id;
    if let Ok(messages) = channel_id.messages(ctx, GetMessages::new().limit(50)).await {
        for msg in messages {
            if msg.author.id == bot_id && !msg.components.is_empty() {
                if let Err(e) = msg.delete(ctx).await {
                    warn!("Failed to delete old bot message: {}", e);
                }
            }
        }
    }

    // Send a welcome back message
    let configure_button = CreateButton::new("configure_channel")
        .label("⚙️ Configure Channel")
        .style(serenity::ButtonStyle::Primary);

    let persistent_button = CreateButton::new("toggle_persistent")
        .label("📌 Remove Persistent")
        .style(serenity::ButtonStyle::Danger);

    let action_row = CreateActionRow::Buttons(vec![configure_button, persistent_button]);

    let message = CreateMessage::new()
        .content(format!(
            "🎙️ **Welcome back to your channel, {}!**\n\n\
            Your persistent channel has been restored from the archive.",
            member.display_name()
        ))
        .components(vec![action_row]);

    channel_id.send_message(ctx, message).await?;

    info!(
        "Restored archived channel {} for user {} in guild {}",
        channel_id, member.user.id, guild_id
    );

    Ok(())
}

/// Check if a user is the owner of a temporary channel
fn is_channel_owner(data: &Data, channel_id: ChannelId, user_id: UserId) -> bool {
    data.temp_channels
        .get(&channel_id)
        .is_some_and(|tc| tc.owner_id == user_id)
}

/// Handle component interactions (button clicks)
async fn handle_interaction(
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
    if !is_channel_owner(data, channel_id, user_id) {
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
            .max_length(100),
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
    if !is_channel_owner(data, channel_id, user_id) {
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
        let has_other_persistent = data.temp_channels.iter().any(|entry| {
            let tc = entry.value();
            tc.owner_id == user_id
                && tc.guild_id == guild_id
                && tc.lobby_channel_id == current_lobby_id
                && tc.is_persistent
                && *entry.key() != channel_id
        });

        if has_other_persistent {
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
async fn handle_modal_submit(
    ctx: &serenity::Context,
    interaction: serenity::ModalInteraction,
    data: &Data,
) {
    if interaction.data.custom_id == "channel_config_modal" {
        if let Err(e) = handle_channel_config_modal(ctx, &interaction, data).await {
            error!("Failed to handle modal submission: {}", e);
        }
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
    if !is_channel_owner(data, channel_id, user_id) {
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

#[tokio::main]
async fn main() {
    // Load environment variables from .env file if present
    let _ = dotenvy::dotenv();

    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("shiibot_rs=info".parse().expect("valid log directive")),
        )
        .init();

    // Get token from environment
    let token = std::env::var("DISCORD_TOKEN")
        .expect("DISCORD_TOKEN environment variable not set. Set it with: export DISCORD_TOKEN=your_bot_token");

    // Get database URL from environment
    let database_url = std::env::var("DATABASE_URL")
        .expect("DATABASE_URL environment variable not set. Set it with: export DATABASE_URL=postgres://user:password@host/database");

    // Connect to database
    let db = Database::new(&database_url)
        .await
        .expect("Failed to connect to database");

    // Load existing data from database
    let lobby_channels = DashMap::new();
    let temp_channels = DashMap::new();
    let archive_categories = DashMap::new();

    // Load lobby channels
    match db.get_all_lobby_channels().await {
        Ok(lobbies) => {
            for (channel_id, guild_id) in lobbies {
                lobby_channels.insert(channel_id, guild_id);
            }
            info!(
                "Loaded {} lobby channels from database",
                lobby_channels.len()
            );
        }
        Err(e) => {
            warn!("Failed to load lobby channels from database: {}", e);
        }
    }

    // Load temp channels
    match db.get_all_temp_channels().await {
        Ok(temps) => {
            for (channel_id, guild_id, owner_id, lobby_channel_id, is_persistent, is_archived) in
                temps
            {
                temp_channels.insert(
                    channel_id,
                    TempChannel {
                        owner_id,
                        lobby_channel_id,
                        is_persistent,
                        is_archived,
                        guild_id,
                    },
                );
            }
            info!("Loaded {} temp channels from database", temp_channels.len());
        }
        Err(e) => {
            warn!("Failed to load temp channels from database: {}", e);
        }
    }

    let data = Data {
        db,
        lobby_channels,
        temp_channels,
        archive_categories,
    };

    // Create framework
    let framework = poise::Framework::builder()
        .options(poise::FrameworkOptions {
            commands: vec![create_lobby(), convert_to_lobby()],
            event_handler: |ctx, event, _framework, data| {
                Box::pin(async move {
                    match event {
                        poise::serenity_prelude::FullEvent::VoiceStateUpdate { old, new } => {
                            handle_voice_state_update(ctx, old.clone(), new.clone(), data).await;
                        }
                        poise::serenity_prelude::FullEvent::InteractionCreate { interaction } => {
                            match interaction {
                                serenity::Interaction::Component(component) => {
                                    handle_interaction(ctx, component.clone(), data).await;
                                }
                                serenity::Interaction::Modal(modal) => {
                                    handle_modal_submit(ctx, modal.clone(), data).await;
                                }
                                _ => {}
                            }
                        }
                        _ => {}
                    }
                    Ok(())
                })
            },
            ..Default::default()
        })
        .setup(move |ctx, _ready, framework| {
            Box::pin(async move {
                // For development: register in a specific guild for instant updates
                // Replace YOUR_GUILD_ID with your test server's ID
                // Uncomment the line below and comment out register_globally:
                // let guild_id = serenity::GuildId::new(YOUR_GUILD_ID);
                // poise::builtins::register_in_guild(ctx, &framework.options().commands, guild_id).await?;
                
                // For production: register globally (takes up to 1 hour to propagate)
                poise::builtins::register_globally(ctx, &framework.options().commands).await?;
                info!("Bot is ready and slash commands registered!");
                Ok(data)
            })
        })
        .build();

    // Create client
    let intents = serenity::GatewayIntents::non_privileged()
        | serenity::GatewayIntents::GUILD_VOICE_STATES
        | serenity::GatewayIntents::GUILD_MEMBERS;

    let mut client = serenity::ClientBuilder::new(token, intents)
        .framework(framework)
        .await
        .expect("Failed to create client");

    // Start the bot
    info!("Starting bot...");
    if let Err(e) = client.start().await {
        error!("Client error: {}", e);
    }
}
