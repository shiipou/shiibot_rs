mod commands;
mod constants;
mod db;
mod handlers;
mod models;

use poise::serenity_prelude as serenity;
use tracing::{error, info};

use crate::{
    commands::{convert_to_lobby, create_lobby},
    constants::LOG_DIRECTIVE,
    db::Database,
    handlers::{handle_interaction, handle_modal_submit, handle_voice_state_update},
    models::Data,
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
    if let Err(e) = start_bot(config.discord_token, data).await {
        error!("Bot error: {}", e);
        std::process::exit(1);
    }
}

/// Configuration loaded from environment variables
struct Config {
    discord_token: String,
    database_url: String,
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

    Ok(Config {
        discord_token,
        database_url,
    })
}

/// Create and start the Discord bot
async fn start_bot(
    token: String,
    data: Data,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
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
