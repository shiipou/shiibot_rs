use poise::serenity_prelude::{ChannelType, GuildChannel, GuildId};

/// Validation error types
#[derive(Debug)]
pub enum ValidationError {
    NotInGuild,
    InvalidChannelType { expected: ChannelType, got: ChannelType },
    ChannelAlreadyExists,
    ChannelIsTemporary,
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ValidationError::NotInGuild => write!(f, "This command must be used in a server"),
            ValidationError::InvalidChannelType { expected, got } => {
                write!(f, "Expected {:?} channel, got {:?}", expected, got)
            }
            ValidationError::ChannelAlreadyExists => write!(f, "Channel already exists"),
            ValidationError::ChannelIsTemporary => write!(f, "Channel is temporary"),
        }
    }
}

impl std::error::Error for ValidationError {}

/// Validate that a channel is of the expected type
pub fn validate_channel_type(
    channel: &GuildChannel,
    expected: ChannelType,
) -> Result<(), ValidationError> {
    if channel.kind != expected {
        return Err(ValidationError::InvalidChannelType {
            expected,
            got: channel.kind,
        });
    }
    Ok(())
}

/// Extract guild ID from context, returning error if not in a guild
pub fn require_guild(guild_id: Option<GuildId>) -> Result<GuildId, ValidationError> {
    guild_id.ok_or(ValidationError::NotInGuild)
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_require_guild() {
        assert!(require_guild(None).is_err());
        assert!(require_guild(Some(GuildId::new(123))).is_ok());
    }
}
