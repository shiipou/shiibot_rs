use poise::serenity_prelude::{
    self as serenity, CreateActionRow, CreateButton, CreateInteractionResponse,
    CreateInteractionResponseMessage, EditChannel, EditInteractionResponse,
};
use tracing::{error, info};

use crate::{
    constants::MAX_CHANNEL_NAME_LENGTH,
    models::{Data, Error},
    utils::string_utils::{is_empty_or_whitespace, take_chars},
    utils::messages::{build_context_error, format_error, format_success},
    utils::channel_utils::is_valid_channel_name,
};

use super::birthday::handle_collect_birthday_button;

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
        "collect_birthday" => {
            if let Err(e) = handle_collect_birthday_button(ctx, &interaction, data).await {
                error!("Failed to handle collect birthday button: {}", e);
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
                .content(format_error("Only the channel owner can configure this channel!"))
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
                .content(format_error("Only the channel owner can change persistence settings!"))
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
        .ok_or_else(|| build_context_error("in temp channels"))?;

    let new_persistent_state = !is_currently_persistent;

    // Check if user already has another persistent channel from the same lobby (only when enabling)
    if new_persistent_state {
        let guild_id = interaction
            .guild_id
            .ok_or_else(|| build_context_error("in a server"))?;

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
                    .content(format_error(
                        "You already have a persistent channel from this lobby! \
                        Please disable persistence on your other channel first.",
                    ))
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
            format_success(
                "**Channel is now persistent!**\n\n\
                When everyone leaves, this channel will be archived instead of deleted.\n\
                When you join the lobby again, your channel will be restored."
            ),
            "📌 Remove Persistent",
            serenity::ButtonStyle::Danger,
        )
    } else {
        (
            format_success(
                "**Channel is no longer persistent.**\n\n\
                When everyone leaves, this channel will be deleted."
            ),
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
    } else if interaction.data.custom_id == "birthday_modal" {
        use super::birthday::handle_birthday_modal;
        if let Err(e) = handle_birthday_modal(ctx, &interaction, data).await {
            error!("Failed to handle birthday modal: {}", e);
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
    if !data.is_channel_owner(channel_id, user_id) {
        let response = CreateInteractionResponse::Message(
            CreateInteractionResponseMessage::new()
                .content(format_error("Only the channel owner can configure this channel!"))
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

    // Validate and sanitize the channel name
    if is_empty_or_whitespace(&new_name) {
        let response = CreateInteractionResponse::Message(
            CreateInteractionResponseMessage::new()
                .content(format_error("Channel name cannot be empty!"))
                .ephemeral(true),
        );
        interaction.create_response(ctx, response).await?;
        return Ok(());
    }
    
    // Validate channel name
    if let Err(validation_error) = is_valid_channel_name(&new_name) {
        let response = CreateInteractionResponse::Message(
            CreateInteractionResponseMessage::new()
                .content(format_error(validation_error))
                .ephemeral(true),
        );
        interaction.create_response(ctx, response).await?;
        return Ok(());
    }
    
    // Truncate to max length
    let sanitized_name = take_chars(&new_name, MAX_CHANNEL_NAME_LENGTH as usize);

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
        .edit(ctx, EditChannel::new().name(&sanitized_name))
        .await?;

    // Send follow-up response
    interaction
        .edit_response(
            ctx,
            EditInteractionResponse::new()
                .content(format_success(&format!("Channel renamed to **{}**!", sanitized_name))),
        )
        .await?;

    info!(
        "User {} renamed temp channel {} to '{}'",
        user_id, channel_id, sanitized_name
    );

    Ok(())
}
