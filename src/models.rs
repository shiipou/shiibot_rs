use dashmap::DashMap;
use poise::serenity_prelude::{ChannelId, GuildId, UserId};
use tokio::sync::watch;

use crate::database::Database;

/// Represents a temporary voice channel owned by a user
#[derive(Clone, Debug)]
pub struct TempChannel {
    pub owner_id: UserId,
    pub lobby_channel_id: ChannelId,
    pub is_persistent: bool,
    pub is_archived: bool,
    pub guild_id: GuildId,
}

/// Bot state shared across all handlers
#[derive(Clone)]
pub struct Data {
    /// Database connection
    pub db: Database,
    /// Maps lobby channel IDs to guild IDs
    pub lobby_channels: DashMap<ChannelId, GuildId>,
    /// Maps temporary channel IDs to their data
    pub temp_channels: DashMap<ChannelId, TempChannel>,
    /// Maps guild IDs to their archive category IDs
    pub archive_categories: DashMap<GuildId, ChannelId>,
    /// Signal to reload schedules
    pub schedule_reload_tx: watch::Sender<u64>,
}

impl Data {
    /// Create a new Data instance with the given database connection
    pub fn new(db: Database) -> Self {
        let (schedule_reload_tx, _) = watch::channel(0);
        Self {
            db,
            lobby_channels: DashMap::new(),
            temp_channels: DashMap::new(),
            archive_categories: DashMap::new(),
            schedule_reload_tx,
        }
    }

    /// Load existing data from the database into memory
    pub async fn load_from_database(&self) -> Result<(), Error> {
        // Load lobby channels
        self.db
            .get_all_lobby_channels()
            .await
            .map(|lobbies| {
                lobbies.into_iter().for_each(|(channel_id, guild_id)| {
                    self.lobby_channels.insert(channel_id, guild_id);
                });
                tracing::info!(
                    "Loaded {} lobby channels from database",
                    self.lobby_channels.len()
                );
            })
            .unwrap_or_else(|e| {
                tracing::warn!("Failed to load lobby channels from database: {}", e);
            });

        // Load temp channels
        self.db
            .get_all_temp_channels()
            .await
            .map(|temps| {
                temps.into_iter().for_each(|(
                    channel_id,
                    guild_id,
                    owner_id,
                    lobby_channel_id,
                    is_persistent,
                    is_archived,
                )| {
                    self.temp_channels.insert(
                        channel_id,
                        TempChannel {
                            owner_id,
                            lobby_channel_id,
                            is_persistent,
                            is_archived,
                            guild_id,
                        },
                    );
                });
                tracing::info!(
                    "Loaded {} temp channels from database",
                    self.temp_channels.len()
                );
            })
            .unwrap_or_else(|e| {
                tracing::warn!("Failed to load temp channels from database: {}", e);
            });

        Ok(())
    }

    /// Check if a user is the owner of a temporary channel
    pub fn is_channel_owner(&self, channel_id: ChannelId, user_id: UserId) -> bool {
        self.temp_channels
            .get(&channel_id)
            .is_some_and(|tc| tc.owner_id == user_id)
    }
}

pub type Error = Box<dyn std::error::Error + Send + Sync>;
pub type Context<'a> = poise::Context<'a, Data, Error>;

#[cfg(test)]
mod tests {
    // Note: is_channel_owner is a pure function but requires DashMap state
    // It would benefit from property-based testing with proptest
    // Tests would require:
    // 1. Create Data instance with mock database
    // 2. Insert temp channel with specific owner
    // 3. Assert is_channel_owner returns true for owner
    // 4. Assert is_channel_owner returns false for non-owner
    // Skipping for now as it requires async database setup
}
