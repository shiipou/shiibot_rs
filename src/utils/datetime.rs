/// Pure date/time utility functions (Discord-agnostic)
use chrono::{Datelike, NaiveDate, Utc};

/// Calculate age from birth year
pub fn calculate_age(birth_year: i32, current_year: i32) -> i32 {
    current_year - birth_year
}

/// Calculate age as of today
pub fn calculate_age_today(birth_year: i32) -> i32 {
    let current_year = Utc::now().year();
    calculate_age(birth_year, current_year)
}

/// Check if a given year is a leap year
pub fn is_leap_year(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
}

/// Check if a date matches month and day (ignoring year)
pub fn matches_birthday(month: i32, day: i32, target_month: i32, target_day: i32) -> bool {
    month == target_month && day == target_day
}

/// Get the current month and day as a tuple
pub fn get_current_month_day() -> (i32, i32) {
    let now = Utc::now();
    (now.month() as i32, now.day() as i32)
}

/// Validate if a month/day combination is valid
pub fn is_valid_date(month: i32, day: i32) -> bool {
    if !(1..=12).contains(&month) {
        return false;
    }

    let max_day = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => 29, // Allow Feb 29 for leap years
        _ => return false,
    };

    (1..=max_day).contains(&day)
}

/// Check if a specific date exists (considering leap years)
pub fn date_exists(year: i32, month: i32, day: i32) -> bool {
    NaiveDate::from_ymd_opt(year, month as u32, day as u32).is_some()
}

/// Format a date as "Day MonthName" (e.g., "15 March")
pub fn format_date_display(month: i32, day: i32) -> String {
    let month_name = get_month_name(month);
    format!("{} {}", day, month_name)
}

/// Get month name from month number (1-12)
pub fn get_month_name(month: i32) -> &'static str {
    match month {
        1 => "January",
        2 => "February",
        3 => "March",
        4 => "April",
        5 => "May",
        6 => "June",
        7 => "July",
        8 => "August",
        9 => "September",
        10 => "October",
        11 => "November",
        12 => "December",
        _ => "Unknown",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_age() {
        assert_eq!(calculate_age(1990, 2025), 35);
        assert_eq!(calculate_age(2000, 2025), 25);
        assert_eq!(calculate_age(1995, 1995), 0);
        assert_eq!(calculate_age(2010, 2025), 15);
    }

    #[test]
    fn test_is_leap_year() {
        assert!(is_leap_year(2000)); // Divisible by 400
        assert!(is_leap_year(2020)); // Divisible by 4, not by 100
        assert!(is_leap_year(2024));
        
        assert!(!is_leap_year(1900)); // Divisible by 100, not by 400
        assert!(!is_leap_year(2100));
        assert!(!is_leap_year(2023)); // Not divisible by 4
    }

    #[test]
    fn test_matches_birthday() {
        assert!(matches_birthday(3, 15, 3, 15));
        assert!(matches_birthday(12, 31, 12, 31));
        
        assert!(!matches_birthday(3, 15, 3, 16));
        assert!(!matches_birthday(3, 15, 4, 15));
        assert!(!matches_birthday(1, 1, 12, 31));
    }

    #[test]
    fn test_is_valid_date() {
        // Valid dates
        assert!(is_valid_date(1, 31));
        assert!(is_valid_date(2, 29)); // Leap day allowed
        assert!(is_valid_date(4, 30));
        assert!(is_valid_date(12, 31));
        
        // Invalid dates
        assert!(!is_valid_date(0, 15)); // Month 0
        assert!(!is_valid_date(13, 15)); // Month 13
        assert!(!is_valid_date(2, 30)); // Feb 30
        assert!(!is_valid_date(4, 31)); // April 31
        assert!(!is_valid_date(6, 0)); // Day 0
        assert!(!is_valid_date(6, 32)); // Day 32
    }

    #[test]
    fn test_date_exists() {
        // Valid dates
        assert!(date_exists(2024, 2, 29)); // Leap year
        assert!(date_exists(2025, 1, 31));
        assert!(date_exists(2025, 4, 30));
        
        // Invalid dates
        assert!(!date_exists(2023, 2, 29)); // Not a leap year
        assert!(!date_exists(2025, 2, 30));
        assert!(!date_exists(2025, 4, 31));
    }

    #[test]
    fn test_format_date_display() {
        assert_eq!(format_date_display(3, 15), "15 March");
        assert_eq!(format_date_display(12, 25), "25 December");
        assert_eq!(format_date_display(1, 1), "1 January");
    }

    #[test]
    fn test_get_month_name() {
        assert_eq!(get_month_name(1), "January");
        assert_eq!(get_month_name(6), "June");
        assert_eq!(get_month_name(12), "December");
        assert_eq!(get_month_name(0), "Unknown");
        assert_eq!(get_month_name(13), "Unknown");
    }

    #[test]
    fn test_get_current_month_day() {
        let (month, day) = get_current_month_day();
        // Just verify they're in valid ranges
        assert!((1..=12).contains(&month));
        assert!((1..=31).contains(&day));
    }
}
