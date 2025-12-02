/// Pure functions for channel name and configuration (Discord-agnostic)

/// Format a temporary channel name for a user
pub fn format_temp_channel_name(user_name: &str) -> String {
    format!("{}'s Channel", user_name)
}

/// Check if a channel name looks like a temporary channel
pub fn is_temp_channel_name(channel_name: &str) -> bool {
    channel_name.ends_with("'s Channel")
}

/// Extract user name from a temporary channel name
pub fn extract_user_from_channel_name(channel_name: &str) -> Option<&str> {
    if !is_temp_channel_name(channel_name) {
        return None;
    }
    
    channel_name.strip_suffix("'s Channel")
}

/// Build archive category name
pub fn build_archive_category_name(base_name: &str) -> String {
    format!("📦 {} Archive", base_name)
}

/// Format a success message for birthday setup
pub fn format_birthday_setup_message(
    channel_name: &str,
    time: &str,
    has_role: bool,
    timezone: &str,
) -> String {
    let role_info = if has_role {
        "\n✅ Birthday role configured"
    } else {
        ""
    };
    
    format!(
        "✅ **Birthday notifications configured!**\n\n\
        📍 Channel: {}\n\
        ⏰ Time: {} ({}){}\n\n\
        Use the button in the collection message to set your birthday!",
        channel_name, time, timezone, role_info
    )
}

/// Format a birthday display string
pub fn format_birthday_display(day: i32, month_name: &str, year: Option<i32>) -> String {
    if let Some(y) = year {
        format!("{} {} {}", day, month_name, y)
    } else {
        format!("{} {}", day, month_name)
    }
}

/// Format a date as MM/DD or MM/DD/YYYY
pub fn format_date_compact(month: i32, day: i32, year: Option<i32>) -> String {
    if let Some(y) = year {
        format!("{:02}/{:02}/{}", month, day, y)
    } else {
        format!("{:02}/{:02}", month, day)
    }
}

/// Validate channel name length and characters
pub fn is_valid_channel_name(name: &str) -> Result<(), &'static str> {
    if name.is_empty() {
        return Err("Channel name cannot be empty");
    }
    
    if name.len() > 100 {
        return Err("Channel name cannot exceed 100 characters");
    }
    
    // Discord channel names have specific character restrictions
    // but we'll keep it simple for now
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_temp_channel_name() {
        assert_eq!(format_temp_channel_name("Alice"), "Alice's Channel");
        assert_eq!(format_temp_channel_name("Bob123"), "Bob123's Channel");
        assert_eq!(format_temp_channel_name("User Name"), "User Name's Channel");
    }

    #[test]
    fn test_is_temp_channel_name() {
        assert!(is_temp_channel_name("Alice's Channel"));
        assert!(is_temp_channel_name("Bob123's Channel"));
        
        assert!(!is_temp_channel_name("General"));
        assert!(!is_temp_channel_name("Alice Channel"));
        assert!(!is_temp_channel_name("Alice's"));
    }

    #[test]
    fn test_extract_user_from_channel_name() {
        assert_eq!(
            extract_user_from_channel_name("Alice's Channel"),
            Some("Alice")
        );
        assert_eq!(
            extract_user_from_channel_name("Bob123's Channel"),
            Some("Bob123")
        );
        
        assert_eq!(extract_user_from_channel_name("General"), None);
        assert_eq!(extract_user_from_channel_name("Alice"), None);
    }

    #[test]
    fn test_build_archive_category_name() {
        assert_eq!(
            build_archive_category_name("Temp Channels"),
            "📦 Temp Channels Archive"
        );
        assert_eq!(
            build_archive_category_name("Voice"),
            "📦 Voice Archive"
        );
    }

    #[test]
    fn test_format_birthday_setup_message() {
        let msg = format_birthday_setup_message(
            "#birthdays",
            "08:00",
            true,
            "America/New_York"
        );
        
        assert!(msg.contains("#birthdays"));
        assert!(msg.contains("08:00"));
        assert!(msg.contains("America/New_York"));
        assert!(msg.contains("Birthday role configured"));
    }

    #[test]
    fn test_format_birthday_setup_message_no_role() {
        let msg = format_birthday_setup_message(
            "#birthdays",
            "09:00",
            false,
            "UTC"
        );
        
        assert!(msg.contains("#birthdays"));
        assert!(!msg.contains("Birthday role configured"));
    }

    #[test]
    fn test_format_birthday_display_with_year() {
        assert_eq!(
            format_birthday_display(15, "March", Some(1990)),
            "15 March 1990"
        );
    }

    #[test]
    fn test_format_birthday_display_without_year() {
        assert_eq!(
            format_birthday_display(15, "March", None),
            "15 March"
        );
    }

    #[test]
    fn test_format_date_compact_with_year() {
        assert_eq!(format_date_compact(3, 15, Some(1990)), "03/15/1990");
        assert_eq!(format_date_compact(12, 1, Some(2000)), "12/01/2000");
    }

    #[test]
    fn test_format_date_compact_without_year() {
        assert_eq!(format_date_compact(3, 15, None), "03/15");
        assert_eq!(format_date_compact(12, 1, None), "12/01");
    }

    #[test]
    fn test_is_valid_channel_name() {
        assert!(is_valid_channel_name("general").is_ok());
        assert!(is_valid_channel_name("my-channel").is_ok());
        assert!(is_valid_channel_name("a").is_ok());
        
        assert!(is_valid_channel_name("").is_err());
        
        let long_name = "a".repeat(101);
        assert!(is_valid_channel_name(&long_name).is_err());
    }
}
