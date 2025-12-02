use poise::serenity_prelude::{ChannelType, CreateChannel, GuildChannel};
use tracing::{error, info};

use crate::{
    constants::DEFAULT_LOBBY_NAME,
    models::{Context, Error},
    utils::messages::{format_error, format_success},
    utils::validation::require_guild,
};

/// Create a lobby voice channel that spawns temporary channels
#[poise::command(slash_command, required_permissions = "MANAGE_CHANNELS")]
pub async fn create_lobby(
    ctx: Context<'_>,
    #[description = "Name for the lobby channel"] name: Option<String>,
) -> Result<(), Error> {
    let guild_id = require_guild(ctx.guild_id())?;

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
        "{}\nLobby: <#{}>\nUsers joining this channel will get their own temporary voice channel!",
        format_success("Lobby channel created!"),
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
    let guild_id = require_guild(ctx.guild_id())?;

    // Verify the channel is a voice channel
    if channel.kind != ChannelType::Voice {
        ctx.say(format_error("The selected channel must be a voice channel!"))
            .await?;
        return Ok(());
    }

    // Check if it's already a lobby
    if ctx.data().lobby_channels.contains_key(&channel.id) {
        ctx.say(format_error("This channel is already a lobby!")).await?;
        return Ok(());
    }

    // Check if it's a temp channel
    if ctx.data().temp_channels.contains_key(&channel.id) {
        ctx.say(format_error("This channel is a temporary channel and cannot be converted to a lobby!"))
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
        "{}\nChannel <#{}> is now a lobby! Users joining will get their own temporary voice channel.",
        format_success("Converted to lobby channel!"),
        channel.id
    ))
    .await?;

    info!(
        "Converted channel {} to lobby in guild {}",
        channel.id, guild_id
    );

    Ok(())
}
