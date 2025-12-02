use poise::serenity_prelude::{
    self as serenity, CreateInteractionResponse, CreateInteractionResponseMessage,
    EditInteractionResponse,
};
use tracing::{error, info};

use crate::models::{Data, Error};
use crate::utils::datetime::{date_exists, get_month_name, is_valid_date};
use crate::utils::channel_utils::format_birthday_display;
use crate::utils::messages::{build_invalid_input_error, build_save_success, format_error};
use crate::utils::string_utils::is_empty_or_whitespace;

/// Pure function: Extract input text from a modal component
fn extract_input_value(
    components: &[serenity::ActionRow],
    index: usize,
) -> Option<String> {
    components
        .get(index)
        .and_then(|row| row.components.first())
        .and_then(|component| match component {
            serenity::ActionRowComponent::InputText(input) => input.value.clone(),
            _ => None,
        })
}

/// Pure function: Parse and validate month
fn parse_month(month_str: &str) -> Result<i32, String> {
    month_str
        .trim()
        .parse::<i32>()
        .ok()
        .filter(|&m| (1..=12).contains(&m))
        .ok_or_else(|| build_invalid_input_error("month", "a number between 1 and 12"))
}

/// Pure function: Parse and validate day
fn parse_day(day_str: &str) -> Result<i32, String> {
    day_str
        .trim()
        .parse::<i32>()
        .ok()
        .filter(|&d| (1..=31).contains(&d))
        .ok_or_else(|| build_invalid_input_error("day", "a number between 1 and 31"))
}

/// Pure function: Parse and validate year (optional)
fn parse_year(year_str: &str) -> Result<Option<i32>, String> {
    if is_empty_or_whitespace(year_str) {
        return Ok(None);
    }
    
    year_str
        .trim()
        .parse::<i32>()
        .ok()
        .filter(|&y| y > 1900 && y <= 2100)
        .map(Some)
        .ok_or_else(|| build_invalid_input_error("year", "a valid year (1901-2100) or leave it empty"))
}

/// Handle the collect birthday button click
pub async fn handle_collect_birthday_button(
    ctx: &serenity::Context,
    interaction: &serenity::ComponentInteraction,
    _data: &Data,
) -> Result<(), Error> {
    // Show modal for birthday input
    let modal =
        serenity::CreateModal::new("birthday_modal", "🎂 Set Your Birthday").components(vec![
            serenity::CreateActionRow::InputText(
                serenity::CreateInputText::new(
                    serenity::InputTextStyle::Short,
                    "Day (1-31)",
                    "birth_day",
                )
                .placeholder("e.g., 15")
                .required(true)
                .min_length(1)
                .max_length(2),
            ),
            serenity::CreateActionRow::InputText(
                serenity::CreateInputText::new(
                    serenity::InputTextStyle::Short,
                    "Month (1-12)",
                    "birth_month",
                )
                .placeholder("e.g., 3 for March")
                .required(true)
                .min_length(1)
                .max_length(2),
            ),
            serenity::CreateActionRow::InputText(
                serenity::CreateInputText::new(
                    serenity::InputTextStyle::Short,
                    "Year (optional)",
                    "birth_year",
                )
                .placeholder("e.g., 1995 (optional)")
                .required(false)
                .min_length(4)
                .max_length(4),
            ),
        ]);

    let response = CreateInteractionResponse::Modal(modal);
    interaction.create_response(ctx, response).await?;

    Ok(())
}

/// Handle the birthday modal submission
pub async fn handle_birthday_modal(
    ctx: &serenity::Context,
    interaction: &serenity::ModalInteraction,
    data: &Data,
) -> Result<(), Error> {
    let user_id = interaction.user.id;

    // Extract values from modal using pure function
    let components = &interaction.data.components;
    
    let day_str = extract_input_value(components, 0).unwrap_or_default();
    let month_str = extract_input_value(components, 1).unwrap_or_default();
    let year_str = extract_input_value(components, 2).unwrap_or_default();

    // Parse and validate using pure functions
    let month = match parse_month(&month_str) {
        Ok(m) => m,
        Err(err_msg) => {
            let response = CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new()
                    .content(&err_msg)
                    .ephemeral(true),
            );
            interaction.create_response(ctx, response).await?;
            return Ok(());
        }
    };

    let day = match parse_day(&day_str) {
        Ok(d) => d,
        Err(err_msg) => {
            let response = CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new()
                    .content(&err_msg)
                    .ephemeral(true),
            );
            interaction.create_response(ctx, response).await?;
            return Ok(());
        }
    };

    let year = match parse_year(&year_str) {
        Ok(y) => y,
        Err(err_msg) => {
            let response = CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new()
                    .content(&err_msg)
                    .ephemeral(true),
            );
            interaction.create_response(ctx, response).await?;
            return Ok(());
        }
    };

    // Validate the date using the pure utility functions
    let is_valid = if let Some(y) = year {
        date_exists(y, month, day)
    } else {
        // For dates without year, validate month/day combination
        is_valid_date(month, day)
    };
    
    if !is_valid {
        let response = CreateInteractionResponse::Message(
            CreateInteractionResponseMessage::new()
                .content(format_error("Invalid date! Please check your month and day combination."))
                .ephemeral(true),
        );
        interaction.create_response(ctx, response).await?;
        return Ok(());
    }

    // Defer the response
    interaction
        .create_response(
            ctx,
            CreateInteractionResponse::Defer(
                CreateInteractionResponseMessage::new().ephemeral(true),
            ),
        )
        .await?;

    // Save to database
    if let Err(e) = data.db.upsert_birthday(user_id, month, day, year).await {
        error!("Failed to save birthday to database: {}", e);
        interaction
            .edit_response(
                ctx,
                EditInteractionResponse::new()
                    .content(format_error("Failed to save your birthday. Please try again later.")),
            )
            .await?;
        return Ok(());
    }

    // Format the birthday message using pure function
    let month_name = get_month_name(month);
    let date_display = format_birthday_display(day, month_name, year);

    interaction
        .edit_response(
            ctx,
            EditInteractionResponse::new().content(format!(
                "{}\n\nYour birthday: {}\n\n\
                This will be used across all servers where this bot is present.",
                build_save_success("Birthday"),
                date_display
            )),
        )
        .await?;

    info!(
        "User {} set birthday to {}/{}/{}",
        user_id,
        month,
        day,
        year.map_or("None".to_string(), |y| y.to_string())
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::datetime::{date_exists, get_month_name, is_leap_year, is_valid_date};

    #[test]
    fn test_extract_input_value() {
        // This is a pure function test - would need mock ActionRow data
        // Skipping as it requires complex Discord types
    }

    #[test]
    fn test_parse_month_valid() {
        assert_eq!(parse_month("1"), Ok(1));
        assert_eq!(parse_month("12"), Ok(12));
        assert_eq!(parse_month("6"), Ok(6));
        assert_eq!(parse_month(" 3 "), Ok(3)); // Test trimming
    }

    #[test]
    fn test_parse_month_invalid() {
        assert!(parse_month("0").is_err());
        assert!(parse_month("13").is_err());
        assert!(parse_month("-1").is_err());
        assert!(parse_month("abc").is_err());
        assert!(parse_month("").is_err());
    }

    #[test]
    fn test_parse_day_valid() {
        assert_eq!(parse_day("1"), Ok(1));
        assert_eq!(parse_day("31"), Ok(31));
        assert_eq!(parse_day("15"), Ok(15));
        assert_eq!(parse_day(" 20 "), Ok(20)); // Test trimming
    }

    #[test]
    fn test_parse_day_invalid() {
        assert!(parse_day("0").is_err());
        assert!(parse_day("32").is_err());
        assert!(parse_day("-5").is_err());
        assert!(parse_day("abc").is_err());
        assert!(parse_day("").is_err());
    }

    #[test]
    fn test_parse_year_valid() {
        assert_eq!(parse_year("1995"), Ok(Some(1995)));
        assert_eq!(parse_year("2000"), Ok(Some(2000)));
        assert_eq!(parse_year("1901"), Ok(Some(1901)));
        assert_eq!(parse_year("2100"), Ok(Some(2100)));
        assert_eq!(parse_year(""), Ok(None)); // Empty is valid
        assert_eq!(parse_year("  "), Ok(None)); // Whitespace only
    }

    #[test]
    fn test_parse_year_invalid() {
        assert!(parse_year("1900").is_err()); // Too old
        assert!(parse_year("2101").is_err()); // Too new
        assert!(parse_year("abc").is_err());
        assert!(parse_year("99").is_err()); // Not 4 digits
    }

    #[test]
    fn test_is_valid_date_valid_dates() {
        assert!(is_valid_date(1, 1)); // January 1st
        assert!(is_valid_date(12, 31)); // December 31st
        assert!(is_valid_date(2, 28)); // Feb 28
        assert!(is_valid_date(2, 29)); // Feb 29 (valid for leap years)
        assert!(is_valid_date(4, 30)); // April 30th
    }

    #[test]
    fn test_is_valid_date_invalid_dates() {
        assert!(!is_valid_date(0, 15)); // Invalid month
        assert!(!is_valid_date(13, 15)); // Invalid month
        assert!(!is_valid_date(6, 0)); // Invalid day
        assert!(!is_valid_date(6, 32)); // Invalid day
        assert!(!is_valid_date(2, 30)); // Feb 30 doesn't exist
        assert!(!is_valid_date(4, 31)); // April 31 doesn't exist
        assert!(!is_valid_date(6, 31)); // June 31 doesn't exist
    }
    
    #[test]
    fn test_date_exists_with_year() {
        assert!(date_exists(2020, 2, 29)); // Feb 29 in leap year
        assert!(!date_exists(2021, 2, 29)); // Feb 29 in non-leap year
        assert!(date_exists(2022, 4, 30)); // April 30
        assert!(!date_exists(2022, 4, 31)); // April 31 doesn't exist
    }

    #[test]
    fn test_is_leap_year() {
        // Leap years
        assert!(is_leap_year(2000)); // Divisible by 400
        assert!(is_leap_year(2020)); // Divisible by 4
        assert!(is_leap_year(2024)); // Divisible by 4
        assert!(is_leap_year(1600)); // Divisible by 400
        
        // Non-leap years
        assert!(!is_leap_year(1900)); // Divisible by 100 but not 400
        assert!(!is_leap_year(2100)); // Divisible by 100 but not 400
        assert!(!is_leap_year(2021)); // Not divisible by 4
        assert!(!is_leap_year(2022)); // Not divisible by 4
    }

    #[test]
    fn test_get_month_name() {
        assert_eq!(get_month_name(1), "January");
        assert_eq!(get_month_name(2), "February");
        assert_eq!(get_month_name(3), "March");
        assert_eq!(get_month_name(4), "April");
        assert_eq!(get_month_name(5), "May");
        assert_eq!(get_month_name(6), "June");
        assert_eq!(get_month_name(7), "July");
        assert_eq!(get_month_name(8), "August");
        assert_eq!(get_month_name(9), "September");
        assert_eq!(get_month_name(10), "October");
        assert_eq!(get_month_name(11), "November");
        assert_eq!(get_month_name(12), "December");
        assert_eq!(get_month_name(0), "Unknown");
        assert_eq!(get_month_name(13), "Unknown");
        assert_eq!(get_month_name(-1), "Unknown");
    }
}
