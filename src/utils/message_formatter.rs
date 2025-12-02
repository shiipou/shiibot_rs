/// Pure functions for birthday message formatting (Discord-agnostic)
use crate::utils::datetime::calculate_age;
use crate::utils::string_utils::process_newlines;

/// Replace placeholders in a message template
pub fn apply_message_template(
    template: &str,
    user_name: &str,
    mention: &str,
    date: &str,
    age: &str,
) -> String {
    let result = template
        .replace("{user}", user_name)
        .replace("{mention}", mention)
        .replace("{date}", date)
        .replace("{age}", age);
    process_newlines(&result)
}

/// Format age information string
pub fn format_age_info(birth_year: Option<i32>, current_year: i32) -> String {
    birth_year
        .map(|year| {
            let age = calculate_age(year, current_year);
            format!(" (turning {})", age)
        })
        .unwrap_or_default()
}

/// Extract age value from age info string (for template replacement)
pub fn extract_age_value(age_info: &str) -> &str {
    age_info
        .trim_start_matches(" (turning ")
        .trim_end_matches(")")
}

/// Build a combined birthday message from parts
pub fn build_combined_message(header: &str, body: &str, footer: &str) -> String {
    format!("{}\n{}\n{}", header, body, footer)
}

/// Build default header for birthday notifications
pub fn build_default_header() -> String {
    "🎉 **Happy Birthday** 🎉\n\nToday we celebrate:".to_string()
}

/// Build default footer for birthday notifications
pub fn build_default_footer() -> String {
    "\nEveryone wish them a happy birthday! 🎂🎈".to_string()
}

/// Process custom text by converting literal \n to actual newlines
pub fn process_custom_text(text: &Option<String>) -> Option<String> {
    text.as_ref().map(|t| t.replace("\\n", "\n"))
}

/// Build a single birthday entry line
pub fn build_birthday_entry(
    user_name: &str,
    mention: &str,
    age_info: &str,
    custom_template_with_age: &Option<String>,
    custom_template_without_age: &Option<String>,
    date: &str,
) -> String {
    let has_age = !age_info.is_empty();
    
    if has_age {
        if let Some(template) = custom_template_with_age {
            let age_value = extract_age_value(age_info);
            apply_message_template(template, user_name, mention, date, age_value)
        } else {
            format!("• {}{}!", mention, age_info)
        }
    } else {
        if let Some(template) = custom_template_without_age {
            apply_message_template(template, user_name, mention, date, "")
        } else {
            format!("• {}!", mention)
        }
    }
}

/// Join multiple birthday entries with newlines
pub fn join_birthday_entries(entries: &[String]) -> String {
    entries.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_apply_message_template() {
        let result = apply_message_template(
            "Happy birthday {user}! You are {age} today.",
            "Alice",
            "<@123>",
            "Jan 1",
            "25",
        );
        assert_eq!(result, "Happy birthday Alice! You are 25 today.");
    }

    #[test]
    fn test_apply_message_template_with_newlines() {
        let result = apply_message_template(
            "Happy birthday {user}!\\nYou are {age}!",
            "Bob",
            "<@456>",
            "Feb 2",
            "30",
        );
        assert_eq!(result, "Happy birthday Bob!\nYou are 30!");
    }

    #[test]
    fn test_apply_message_template_with_mention() {
        let result = apply_message_template(
            "🎉 {mention} is turning {age} today on {date}!",
            "Charlie",
            "<@789>",
            "15 March",
            "20",
        );
        assert_eq!(result, "🎉 <@789> is turning 20 today on 15 March!");
    }

    #[test]
    fn test_format_age_info_with_year() {
        assert_eq!(format_age_info(Some(2000), 2025), " (turning 25)");
        assert_eq!(format_age_info(Some(1990), 2025), " (turning 35)");
    }

    #[test]
    fn test_format_age_info_without_year() {
        assert_eq!(format_age_info(None, 2025), "");
    }

    #[test]
    fn test_extract_age_value() {
        assert_eq!(extract_age_value(" (turning 25)"), "25");
        assert_eq!(extract_age_value(" (turning 0)"), "0");
        assert_eq!(extract_age_value(""), "");
    }

    #[test]
    fn test_build_combined_message() {
        let result = build_combined_message("Header", "Body content", "Footer");
        assert_eq!(result, "Header\nBody content\nFooter");
    }

    #[test]
    fn test_build_default_header() {
        let header = build_default_header();
        assert!(header.contains("Happy Birthday"));
        assert!(header.contains("🎉"));
    }

    #[test]
    fn test_build_default_footer() {
        let footer = build_default_footer();
        assert!(footer.contains("wish them a happy birthday"));
        assert!(footer.contains("🎂"));
    }

    #[test]
    fn test_process_custom_text() {
        assert_eq!(
            process_custom_text(&Some("Line 1\\nLine 2".to_string())),
            Some("Line 1\nLine 2".to_string())
        );
        assert_eq!(process_custom_text(&None), None);
    }

    #[test]
    fn test_build_birthday_entry_with_template() {
        let entry = build_birthday_entry(
            "Alice",
            "<@123>",
            " (turning 25)",
            &Some("{user} ({age})".to_string()),
            &Some("{user}".to_string()),
            "15 March",
        );
        assert_eq!(entry, "Alice (25)");
    }

    #[test]
    fn test_build_birthday_entry_default() {
        let entry = build_birthday_entry(
            "Bob",
            "<@456>",
            " (turning 30)",
            &None,
            &None,
            "20 April",
        );
        assert_eq!(entry, "• <@456> (turning 30)!");
    }

    #[test]
    fn test_build_birthday_entry_no_age() {
        let entry = build_birthday_entry(
            "Charlie",
            "<@789>",
            "",
            &Some("{user} ({age})".to_string()),
            &Some("{mention} celebrates today!".to_string()),
            "1 January",
        );
        assert_eq!(entry, "<@789> celebrates today!");
    }

    #[test]
    fn test_join_birthday_entries() {
        let entries = vec![
            "• <@123> (turning 25)!".to_string(),
            "• <@456> (turning 30)!".to_string(),
            "• <@789>!".to_string(),
        ];
        let result = join_birthday_entries(&entries);
        assert_eq!(
            result,
            "• <@123> (turning 25)!\n• <@456> (turning 30)!\n• <@789>!"
        );
    }

    #[test]
    fn test_join_birthday_entries_single() {
        let entries = vec!["• <@123>!".to_string()];
        let result = join_birthday_entries(&entries);
        assert_eq!(result, "• <@123>!");
    }

    #[test]
    fn test_join_birthday_entries_empty() {
        let entries: Vec<String> = vec![];
        let result = join_birthday_entries(&entries);
        assert_eq!(result, "");
    }
}
