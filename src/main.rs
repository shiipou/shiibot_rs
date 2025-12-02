mod commands;
mod commands_timezone;
mod constants;
mod db;
mod handlers;
mod models;
mod schedule;

use poise::serenity_prelude as serenity;
use std::sync::Arc;
use tracing::{error, info};

use crate::{
    commands::{convert_to_lobby, create_lobby, disable_birthday, setup_birthday},
    commands_timezone::setup_timezone,
    constants::LOG_DIRECTIVE,
    db::Database,
    handlers::{handle_interaction, handle_modal_submit, handle_voice_state_update},
    models::Data,
    schedule::start_schedule_manager,
};

#[tokio::main]
async fn main() {
    // Load environment variables from .env file if present
    let _ = dotenvy::dotenv();

    // Initialize logging
    initialize_logging();

    // Load configuration from environment
    let config = match load_configuration() {
        Ok(config) => config,
        Err(e) => {
            error!("Failed to load configuration: {}", e);
            std::process::exit(1);
        }
    };

    // Connect to database
    let db = match Database::new(&config.database_url).await {
        Ok(db) => db,
        Err(e) => {
            error!("Failed to connect to database: {}", e);
            std::process::exit(1);
        }
    };

    // Initialize bot data
    let data = Data::new(db);

    // Load existing data from database
    if let Err(e) = data.load_from_database().await {
        error!("Failed to load data from database: {}", e);
    }

    // Create and start the bot
    if let Err(e) = start_bot(config.discord_token, data, config.dev_guild_id).await {
        error!("Bot error: {}", e);
        std::process::exit(1);
    }
}

/// Configuration loaded from environment variables
struct Config {
    discord_token: String,
    database_url: String,
    dev_guild_id: Option<u64>,
}

/// Initialize the logging system
fn initialize_logging() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(LOG_DIRECTIVE.parse().expect("valid log directive")),
        )
        .init();
}

/// Load configuration from environment variables
fn load_configuration() -> Result<Config, Box<dyn std::error::Error>> {
    let discord_token = std::env::var("DISCORD_TOKEN")
        .map_err(|_| "DISCORD_TOKEN environment variable not set. Set it with: export DISCORD_TOKEN=your_bot_token")?;

    let database_url = std::env::var("DATABASE_URL")
        .map_err(|_| "DATABASE_URL environment variable not set. Set it with: export DATABASE_URL=postgres://user:password@host/database")?;

    // Optional: development guild ID for faster command registration
    let dev_guild_id = std::env::var("DEV_GUILD_ID")
        .ok()
        .and_then(|id| id.parse::<u64>().ok());

    if dev_guild_id.is_some() {
        info!("Development mode: Commands will be registered to guild only");
    }

    Ok(Config {
        discord_token,
        database_url,
        dev_guild_id,
    })
}

/// Create and start the Discord bot
async fn start_bot(
    token: String,
    data: Data,
    dev_guild_id: Option<u64>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Wrap data in Arc for sharing with birthday checker
    let data_arc = Arc::new(data);
    let data_for_framework = Arc::clone(&data_arc);

    // Create framework
    let framework = poise::Framework::builder()
        .options(poise::FrameworkOptions {
            commands: vec![
                create_lobby(),
                convert_to_lobby(),
                setup_birthday(),
                disable_birthday(),
                setup_timezone(),
            ],
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
            let http = ctx.http.clone();
            let cache = ctx.cache.clone();
            let data_clone = Arc::clone(&data_for_framework);

            // Start schedule manager
            start_schedule_manager(http, cache, data_clone);
            info!("Schedule manager task started");

            Box::pin(async move {
                // Register commands based on dev_guild_id
                if let Some(guild_id) = dev_guild_id {
                    let guild = serenity::GuildId::new(guild_id);
                    info!("Registering commands in development guild: {}", guild_id);
                    poise::builtins::register_in_guild(ctx, &framework.options().commands, guild)
                        .await?;
                    info!(
                        "Commands registered in guild {} (instant updates)",
                        guild_id
                    );
                } else {
                    info!("Registering commands globally (may take up to 1 hour)");
                    poise::builtins::register_globally(ctx, &framework.options().commands).await?;
                    info!("Commands registered globally");
                }

                info!("Bot is ready!");

                // Return a new clone of the data
                Ok((*data_for_framework).clone())
            })
        })
        .build();

    // Create client with required intents
    let intents = serenity::GatewayIntents::non_privileged()
        | serenity::GatewayIntents::GUILD_VOICE_STATES
        | serenity::GatewayIntents::GUILD_MEMBERS;

    let mut client = serenity::ClientBuilder::new(token, intents)
        .framework(framework)
        .await?;

    // Start the bot
    info!("Starting bot...");
    client.start().await?;

    Ok(())
}
