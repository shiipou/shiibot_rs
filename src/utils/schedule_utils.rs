/// Pure cron and schedule calculation utilities (Discord-agnostic)
use std::cmp::Ordering;

/// Parse a cron expression and validate basic structure
/// Returns true if the cron expression has valid format (6 fields)
pub fn is_valid_cron_format(cron_expr: &str) -> bool {
    let parts: Vec<&str> = cron_expr.split_whitespace().collect();
    parts.len() == 6
}

/// Extract hour from a cron expression (assumes valid format)
/// Cron format: "second minute hour day month weekday"
pub fn extract_cron_hour(cron_expr: &str) -> Option<u32> {
    let parts: Vec<&str> = cron_expr.split_whitespace().collect();
    if parts.len() >= 3 {
        parts[2].parse().ok()
    } else {
        None
    }
}

/// Extract minute from a cron expression (assumes valid format)
pub fn extract_cron_minute(cron_expr: &str) -> Option<u32> {
    let parts: Vec<&str> = cron_expr.split_whitespace().collect();
    if parts.len() >= 2 {
        parts[1].parse().ok()
    } else {
        None
    }
}

/// Format time as HH:MM
pub fn format_time_hhmm(hour: u32, minute: u32) -> String {
    format!("{:02}:{:02}", hour, minute)
}

/// Calculate minutes until a target time (same day)
/// Returns None if target is in the past
pub fn minutes_until_time(current_hour: u32, current_minute: u32, target_hour: u32, target_minute: u32) -> Option<i64> {
    let current_total = (current_hour * 60 + current_minute) as i64;
    let target_total = (target_hour * 60 + target_minute) as i64;
    
    let diff = target_total - current_total;
    if diff > 0 {
        Some(diff)
    } else {
        None
    }
}

/// Calculate minutes until a target time, wrapping to next day if needed
pub fn minutes_until_time_with_wrap(current_hour: u32, current_minute: u32, target_hour: u32, target_minute: u32) -> i64 {
    let current_total = (current_hour * 60 + current_minute) as i64;
    let target_total = (target_hour * 60 + target_minute) as i64;
    
    let diff = target_total - current_total;
    if diff > 0 {
        diff
    } else {
        // Add 24 hours (1440 minutes) for next day
        diff + 1440
    }
}

/// Compare two durations and return the shorter one
pub fn min_duration(a: i64, b: i64) -> i64 {
    match a.cmp(&b) {
        Ordering::Less | Ordering::Equal => a,
        Ordering::Greater => b,
    }
}

/// Check if a schedule is enabled
pub fn is_schedule_enabled(enabled: bool) -> bool {
    enabled
}

/// Filter enabled schedules
pub fn filter_enabled<T>(items: Vec<(T, bool)>) -> Vec<T> {
    items
        .into_iter()
        .filter(|(_, enabled)| *enabled)
        .map(|(item, _)| item)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_valid_cron_format() {
        assert!(is_valid_cron_format("0 30 8 * * *"));
        assert!(is_valid_cron_format("0 0 0 * * MON"));
        
        assert!(!is_valid_cron_format("0 30 8 * *")); // Only 5 fields
        assert!(!is_valid_cron_format("invalid"));
    }

    #[test]
    fn test_extract_cron_hour() {
        assert_eq!(extract_cron_hour("0 30 8 * * *"), Some(8));
        assert_eq!(extract_cron_hour("0 0 23 * * *"), Some(23));
        assert_eq!(extract_cron_hour("invalid"), None);
    }

    #[test]
    fn test_extract_cron_minute() {
        assert_eq!(extract_cron_minute("0 30 8 * * *"), Some(30));
        assert_eq!(extract_cron_minute("0 45 12 * * *"), Some(45));
        assert_eq!(extract_cron_minute("invalid"), None);
    }

    #[test]
    fn test_format_time_hhmm() {
        assert_eq!(format_time_hhmm(8, 30), "08:30");
        assert_eq!(format_time_hhmm(23, 5), "23:05");
        assert_eq!(format_time_hhmm(0, 0), "00:00");
    }

    #[test]
    fn test_minutes_until_time() {
        // Current: 08:00, Target: 09:30 = 90 minutes
        assert_eq!(minutes_until_time(8, 0, 9, 30), Some(90));
        
        // Current: 10:15, Target: 10:45 = 30 minutes
        assert_eq!(minutes_until_time(10, 15, 10, 45), Some(30));
        
        // Target in the past (same day)
        assert_eq!(minutes_until_time(10, 0, 9, 0), None);
    }

    #[test]
    fn test_minutes_until_time_with_wrap() {
        // Current: 08:00, Target: 09:30 = 90 minutes
        assert_eq!(minutes_until_time_with_wrap(8, 0, 9, 30), 90);
        
        // Current: 23:00, Target: 01:00 = 120 minutes (next day)
        assert_eq!(minutes_until_time_with_wrap(23, 0, 1, 0), 120);
        
        // Current: 10:00, Target: 09:00 = 1380 minutes (23 hours)
        assert_eq!(minutes_until_time_with_wrap(10, 0, 9, 0), 1380);
    }

    #[test]
    fn test_min_duration() {
        assert_eq!(min_duration(100, 200), 100);
        assert_eq!(min_duration(300, 150), 150);
        assert_eq!(min_duration(50, 50), 50);
    }

    #[test]
    fn test_is_schedule_enabled() {
        assert!(is_schedule_enabled(true));
        assert!(!is_schedule_enabled(false));
    }

    #[test]
    fn test_filter_enabled() {
        let items = vec![
            ("item1", true),
            ("item2", false),
            ("item3", true),
            ("item4", false),
        ];
        
        let enabled = filter_enabled(items);
        assert_eq!(enabled, vec!["item1", "item3"]);
    }

    #[test]
    fn test_filter_enabled_all_disabled() {
        let items = vec![("item1", false), ("item2", false)];
        let enabled = filter_enabled(items);
        assert_eq!(enabled.len(), 0);
    }

    #[test]
    fn test_filter_enabled_all_enabled() {
        let items = vec![("item1", true), ("item2", true)];
        let enabled = filter_enabled(items);
        assert_eq!(enabled.len(), 2);
    }
}
