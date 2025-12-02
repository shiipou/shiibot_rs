use chrono::{LocalResult, NaiveTime, TimeZone, Timelike};
use chrono_tz::Tz;

/// Error types for timezone operations
#[derive(Debug)]
pub enum TimezoneError {
    InvalidTimezone(String),
    InvalidTime(String),
    TimeDoesNotExist,
}

impl std::fmt::Display for TimezoneError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TimezoneError::InvalidTimezone(tz) => write!(f, "Invalid timezone: {}", tz),
            TimezoneError::InvalidTime(msg) => write!(f, "Invalid time format: {}", msg),
            TimezoneError::TimeDoesNotExist => write!(f, "Time doesn't exist in this timezone (DST transition)"),
        }
    }
}

impl std::error::Error for TimezoneError {}

/// Convert a naive time in a specific timezone to UTC time
/// Returns (UTC hour, UTC minute) as used in cron expressions
pub fn convert_local_time_to_utc(
    time: NaiveTime,
    timezone: &Tz,
) -> Result<NaiveTime, TimezoneError> {
    let today = chrono::Utc::now().date_naive();
    let local_datetime = today.and_time(time);
    
    // Handle potential DST ambiguity
    let local_datetime_tz = match timezone.from_local_datetime(&local_datetime) {
        LocalResult::Single(dt) => dt,
        LocalResult::Ambiguous(dt1, _dt2) => dt1, // Use earliest during DST transition
        LocalResult::None => return Err(TimezoneError::TimeDoesNotExist),
    };
    
    let utc_datetime = local_datetime_tz.with_timezone(&chrono::Utc);
    Ok(utc_datetime.time())
}

/// Parse a timezone string
pub fn parse_timezone(tz_str: &str) -> Result<Tz, TimezoneError> {
    tz_str.parse().map_err(|_| TimezoneError::InvalidTimezone(tz_str.to_string()))
}

/// Parse a time string in HH:MM format
pub fn parse_time_string(time_str: &str) -> Result<NaiveTime, TimezoneError> {
    NaiveTime::parse_from_str(time_str, "%H:%M")
        .map_err(|_| TimezoneError::InvalidTime(format!("Expected HH:MM format, got '{}'", time_str)))
}

/// Create a cron expression from UTC time
pub fn create_cron_expression(utc_time: NaiveTime) -> String {
    format!(
        "0 {} {} * * *",
        utc_time.minute(),
        utc_time.hour()
    )
}

/// Convert local time string to UTC cron expression
pub fn local_time_to_cron(
    time_str: &str,
    timezone_str: &str,
) -> Result<(String, NaiveTime), TimezoneError> {
    let parsed_time = parse_time_string(time_str)?;
    let tz = parse_timezone(timezone_str)?;
    let utc_time = convert_local_time_to_utc(parsed_time, &tz)?;
    let cron_expr = create_cron_expression(utc_time);
    
    Ok((cron_expr, utc_time))
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_parse_time_string() {
        assert!(parse_time_string("08:00").is_ok());
        assert!(parse_time_string("23:59").is_ok());
        assert!(parse_time_string("invalid").is_err());
    }
    
    #[test]
    fn test_parse_timezone() {
        assert!(parse_timezone("UTC").is_ok());
        assert!(parse_timezone("Europe/Paris").is_ok());
        assert!(parse_timezone("Invalid/Timezone").is_err());
    }
    
    #[test]
    fn test_create_cron_expression() {
        let time = NaiveTime::from_hms_opt(8, 30, 0).unwrap();
        let cron = create_cron_expression(time);
        assert_eq!(cron, "0 30 8 * * *");
    }
}
