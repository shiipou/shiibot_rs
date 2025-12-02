/// Pure string processing utilities (Discord-agnostic)

/// Replace literal \n with actual newlines
pub fn process_newlines(text: &str) -> String {
    text.replace("\\n", "\n")
}

/// Trim and normalize whitespace in a string
pub fn normalize_whitespace(text: &str) -> String {
    text.trim()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Extract the first N characters of a string
pub fn take_chars(text: &str, n: usize) -> String {
    text.chars().take(n).collect()
}

/// Check if a string is empty after trimming
pub fn is_empty_or_whitespace(text: &str) -> bool {
    text.trim().is_empty()
}

/// Convert a string to a safe identifier (lowercase, alphanumeric + underscore)
pub fn to_safe_identifier(text: &str) -> String {
    text.chars()
        .filter_map(|c| {
            if c.is_alphanumeric() || c == '_' {
                Some(c.to_ascii_lowercase())
            } else if c.is_whitespace() || c == '-' {
                Some('_')
            } else {
                None
            }
        })
        .collect()
}

/// Split a string by delimiter and trim each part
pub fn split_and_trim(text: &str, delimiter: char) -> Vec<String> {
    text.split(delimiter)
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

/// Join strings with a separator, filtering out empty strings
pub fn join_non_empty(parts: &[String], separator: &str) -> String {
    parts
        .iter()
        .filter(|s| !s.is_empty())
        .map(|s| s.as_str())
        .collect::<Vec<_>>()
        .join(separator)
}

/// Check if a string contains any of the given substrings
pub fn contains_any(text: &str, patterns: &[&str]) -> bool {
    patterns.iter().any(|pattern| text.contains(pattern))
}

/// Check if a string starts with any of the given prefixes
pub fn starts_with_any(text: &str, prefixes: &[&str]) -> bool {
    prefixes.iter().any(|prefix| text.starts_with(prefix))
}

/// Repeat a string n times
pub fn repeat_string(text: &str, count: usize) -> String {
    text.repeat(count)
}

/// Pad a string to a specific length with a character
pub fn pad_left(text: &str, total_length: usize, pad_char: char) -> String {
    if text.len() >= total_length {
        text.to_string()
    } else {
        let padding = repeat_string(&pad_char.to_string(), total_length - text.len());
        format!("{}{}", padding, text)
    }
}

/// Pad a string to the right
pub fn pad_right(text: &str, total_length: usize, pad_char: char) -> String {
    if text.len() >= total_length {
        text.to_string()
    } else {
        let padding = repeat_string(&pad_char.to_string(), total_length - text.len());
        format!("{}{}", text, padding)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_process_newlines() {
        assert_eq!(process_newlines("Hello\\nWorld"), "Hello\nWorld");
        assert_eq!(process_newlines("Line1\\nLine2\\nLine3"), "Line1\nLine2\nLine3");
        assert_eq!(process_newlines("No newlines"), "No newlines");
    }

    #[test]
    fn test_normalize_whitespace() {
        assert_eq!(normalize_whitespace("  hello   world  "), "hello world");
        assert_eq!(normalize_whitespace("multiple   spaces"), "multiple spaces");
        assert_eq!(normalize_whitespace("  "), "");
    }

    #[test]
    fn test_take_chars() {
        assert_eq!(take_chars("Hello World", 5), "Hello");
        assert_eq!(take_chars("Short", 10), "Short");
        assert_eq!(take_chars("Test", 0), "");
    }

    #[test]
    fn test_is_empty_or_whitespace() {
        assert!(is_empty_or_whitespace(""));
        assert!(is_empty_or_whitespace("   "));
        assert!(is_empty_or_whitespace("\t\n"));
        
        assert!(!is_empty_or_whitespace("text"));
        assert!(!is_empty_or_whitespace("  text  "));
    }

    #[test]
    fn test_to_safe_identifier() {
        assert_eq!(to_safe_identifier("Hello World"), "hello_world");
        assert_eq!(to_safe_identifier("Test-Name"), "test_name");
        assert_eq!(to_safe_identifier("With123Numbers"), "with123numbers");
        assert_eq!(to_safe_identifier("Special@#$Chars"), "specialchars");
    }

    #[test]
    fn test_split_and_trim() {
        assert_eq!(
            split_and_trim("apple, banana, cherry", ','),
            vec!["apple", "banana", "cherry"]
        );
        assert_eq!(
            split_and_trim("one  ,  two  , three", ','),
            vec!["one", "two", "three"]
        );
        assert_eq!(split_and_trim("single", ','), vec!["single"]);
    }

    #[test]
    fn test_join_non_empty() {
        let parts = vec!["hello".to_string(), "".to_string(), "world".to_string()];
        assert_eq!(join_non_empty(&parts, " "), "hello world");
        
        let all_empty = vec!["".to_string(), "".to_string()];
        assert_eq!(join_non_empty(&all_empty, " "), "");
    }

    #[test]
    fn test_contains_any() {
        assert!(contains_any("hello world", &["world", "test"]));
        assert!(contains_any("hello world", &["hello"]));
        
        assert!(!contains_any("hello world", &["foo", "bar"]));
    }

    #[test]
    fn test_starts_with_any() {
        assert!(starts_with_any("hello world", &["hello", "test"]));
        assert!(starts_with_any("test string", &["foo", "test"]));
        
        assert!(!starts_with_any("hello world", &["world", "foo"]));
    }

    #[test]
    fn test_repeat_string() {
        assert_eq!(repeat_string("ab", 3), "ababab");
        assert_eq!(repeat_string("x", 5), "xxxxx");
        assert_eq!(repeat_string("test", 0), "");
    }

    #[test]
    fn test_pad_left() {
        assert_eq!(pad_left("42", 5, '0'), "00042");
        assert_eq!(pad_left("test", 3, ' '), "test");
        assert_eq!(pad_left("hi", 5, '*'), "***hi");
    }

    #[test]
    fn test_pad_right() {
        assert_eq!(pad_right("42", 5, '0'), "42000");
        assert_eq!(pad_right("test", 3, ' '), "test");
        assert_eq!(pad_right("hi", 5, '*'), "hi***");
    }
}
