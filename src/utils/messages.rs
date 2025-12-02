/// Pure functions for formatting error and success messages (Discord-agnostic)

/// Format a validation error message with emoji
pub fn format_error(message: &str) -> String {
    format!("❌ {}", message)
}

/// Format a success message with emoji
pub fn format_success(message: &str) -> String {
    format!("✅ {}", message)
}

/// Format a warning message with emoji
pub fn format_warning(message: &str) -> String {
    format!("⚠️ {}", message)
}

/// Format an info message with emoji
pub fn format_info(message: &str) -> String {
    format!("ℹ️ {}", message)
}

/// Build an error message for invalid input
pub fn build_invalid_input_error(field_name: &str, expected: &str) -> String {
    format_error(&format!(
        "Invalid {}! Please enter {}.",
        field_name, expected
    ))
}

/// Build an error message for missing permissions
pub fn build_permission_error(required_permission: &str) -> String {
    format_error(&format!(
        "You don't have permission to do this. Required: {}",
        required_permission
    ))
}

/// Build an error message for command usage in wrong context
pub fn build_context_error(required_context: &str) -> String {
    format_error(&format!(
        "This command must be used {}",
        required_context
    ))
}

/// Build a database error message (generic, doesn't expose internals)
pub fn build_database_error() -> String {
    format_error("A database error occurred. Please try again later.")
}

/// Build a success message for saving data
pub fn build_save_success(item_type: &str) -> String {
    format_success(&format!("{} saved successfully!", item_type))
}

/// Build a success message for deleting data
pub fn build_delete_success(item_type: &str) -> String {
    format_success(&format!("{} deleted successfully!", item_type))
}

/// Build a help text for time format
pub fn build_time_format_help() -> String {
    "Time must be in HH:MM format (24-hour), e.g., 08:00 or 15:30".to_string()
}

/// Build a help text for date format
pub fn build_date_format_help() -> String {
    "Date must be in MM/DD format or MM/DD/YYYY format, e.g., 03/15 or 03/15/1990".to_string()
}

/// Truncate a long message with ellipsis
pub fn truncate_message(message: &str, max_length: usize) -> String {
    if message.len() <= max_length {
        message.to_string()
    } else if max_length < 3 {
        message.chars().take(max_length).collect()
    } else {
        let truncated: String = message.chars().take(max_length - 3).collect();
        format!("{}...", truncated)
    }
}

/// Join multiple error messages into one
pub fn join_errors(errors: &[String]) -> String {
    if errors.is_empty() {
        return String::new();
    }
    
    if errors.len() == 1 {
        return errors[0].clone();
    }
    
    format!(
        "Multiple errors occurred:\n{}",
        errors
            .iter()
            .enumerate()
            .map(|(i, err)| format!("{}. {}", i + 1, err))
            .collect::<Vec<_>>()
            .join("\n")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_error() {
        assert_eq!(format_error("Something failed"), "❌ Something failed");
    }

    #[test]
    fn test_format_success() {
        assert_eq!(format_success("It worked"), "✅ It worked");
    }

    #[test]
    fn test_format_warning() {
        assert_eq!(format_warning("Be careful"), "⚠️ Be careful");
    }

    #[test]
    fn test_format_info() {
        assert_eq!(format_info("Good to know"), "ℹ️ Good to know");
    }

    #[test]
    fn test_build_invalid_input_error() {
        let result = build_invalid_input_error("month", "a number between 1 and 12");
        assert!(result.contains("❌"));
        assert!(result.contains("month"));
        assert!(result.contains("1 and 12"));
    }

    #[test]
    fn test_build_permission_error() {
        let result = build_permission_error("MANAGE_GUILD");
        assert!(result.contains("❌"));
        assert!(result.contains("permission"));
        assert!(result.contains("MANAGE_GUILD"));
    }

    #[test]
    fn test_build_context_error() {
        let result = build_context_error("in a server");
        assert!(result.contains("❌"));
        assert!(result.contains("in a server"));
    }

    #[test]
    fn test_build_database_error() {
        let result = build_database_error();
        assert!(result.contains("❌"));
        assert!(result.contains("database"));
    }

    #[test]
    fn test_build_save_success() {
        assert_eq!(build_save_success("Birthday"), "✅ Birthday saved successfully!");
    }

    #[test]
    fn test_build_delete_success() {
        assert_eq!(build_delete_success("Channel"), "✅ Channel deleted successfully!");
    }

    #[test]
    fn test_build_time_format_help() {
        let help = build_time_format_help();
        assert!(help.contains("HH:MM"));
        assert!(help.contains("24-hour"));
    }

    #[test]
    fn test_build_date_format_help() {
        let help = build_date_format_help();
        assert!(help.contains("MM/DD"));
    }

    #[test]
    fn test_truncate_message_short() {
        assert_eq!(truncate_message("Hello", 10), "Hello");
    }

    #[test]
    fn test_truncate_message_long() {
        assert_eq!(
            truncate_message("This is a very long message", 10),
            "This is..."
        );
    }

    #[test]
    fn test_truncate_message_exact() {
        assert_eq!(truncate_message("Hello", 5), "Hello");
    }

    #[test]
    fn test_truncate_message_very_short_limit() {
        assert_eq!(truncate_message("Hello", 2), "He");
        assert_eq!(truncate_message("Hello", 0), "");
    }

    #[test]
    fn test_join_errors_empty() {
        let errors: Vec<String> = vec![];
        assert_eq!(join_errors(&errors), "");
    }

    #[test]
    fn test_join_errors_single() {
        let errors = vec!["Error 1".to_string()];
        assert_eq!(join_errors(&errors), "Error 1");
    }

    #[test]
    fn test_join_errors_multiple() {
        let errors = vec![
            "First error".to_string(),
            "Second error".to_string(),
            "Third error".to_string(),
        ];
        let result = join_errors(&errors);
        
        assert!(result.contains("Multiple errors"));
        assert!(result.contains("1. First error"));
        assert!(result.contains("2. Second error"));
        assert!(result.contains("3. Third error"));
    }
}
