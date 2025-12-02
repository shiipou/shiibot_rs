/// Birthday service - handles business logic for birthday notifications
use chrono::Datelike;
use poise::serenity_prelude::{ChannelId, GuildId, RoleId, UserId};

use crate::database::Database;

/// Configuration for setting up birthday notifications in a guild
#[derive(Debug, Clone)]
pub struct BirthdaySetup {
    pub guild_id: GuildId,
    pub notification_channel: ChannelId,
    pub notification_time: String, // HH:MM format
    pub timezone: String,
    pub birthday_role: Option<RoleId>,
    pub custom_message: Option<String>,
    pub custom_header: Option<String>,
    pub custom_footer: Option<String>,
    pub collection_title: Option<String>,
    pub collection_description: Option<String>,
    pub collection_button_label: Option<String>,
}

/// Birthday data for a user
#[derive(Debug, Clone)]
pub struct UserBirthday {
    pub user_id: UserId,
    pub month: i32,
    pub day: i32,
    pub year: Option<i32>,
}

impl UserBirthday {
    /// Calculate age if birth year is known
    pub fn age_on_date(&self, year: i32) -> Option<u32> {
        self.year.map(|birth_year| (year - birth_year) as u32)
    }

    /// Format birthday for display
    pub fn formatted_date(&self) -> String {
        if let Some(year) = self.year {
            format!("{:02}/{:02}/{}", self.day, self.month, year)
        } else {
            format!("{:02}/{:02}", self.day, self.month)
        }
    }
}

/// Service for birthday-related operations
pub struct BirthdayService<'a> {
    db: &'a Database,
}

impl<'a> BirthdayService<'a> {
    /// Create a new birthday service
    pub fn new(db: &'a Database) -> Self {
        Self { db }
    }

    /// Save a user's birthday
    pub async fn save_birthday(
        &self,
        birthday: UserBirthday,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Validate birthday
        Self::validate_birthday_date(birthday.month, birthday.day)?;

        if let Some(year) = birthday.year {
            Self::validate_birth_year(year)?;
        }

        self.db
            .upsert_birthday(birthday.user_id, birthday.month, birthday.day, birthday.year)
            .await?;

        Ok(())
    }

    /// Get a user's birthday
    pub async fn get_birthday(
        &self,
        user_id: UserId,
    ) -> Result<Option<UserBirthday>, Box<dyn std::error::Error>> {
        let result = self.db.get_birthday(user_id).await?;

        Ok(result.map(|(month, day, year)| UserBirthday {
            user_id,
            month,
            day,
            year,
        }))
    }

    /// Get all users with birthdays today
    pub async fn get_todays_birthdays(
        &self,
    ) -> Result<Vec<UserBirthday>, Box<dyn std::error::Error>> {
        let now = chrono::Utc::now();
        let month = now.month() as i32;
        let day = now.day() as i32;

        let users = self.db.get_birthdays_on_date(month, day).await?;

        Ok(users
            .into_iter()
            .map(|(user_id, year)| UserBirthday {
                user_id,
                month,
                day,
                year,
            })
            .collect())
    }

    /// Validate birthday date (month and day)
    fn validate_birthday_date(month: i32, day: i32) -> Result<(), &'static str> {
        if !(1..=12).contains(&month) {
            return Err("Month must be between 1 and 12");
        }

        let max_day = match month {
            1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
            4 | 6 | 9 | 11 => 30,
            2 => 29, // Allow Feb 29 for leap years
            _ => return Err("Invalid month"),
        };

        if !(1..=max_day).contains(&day) {
            return Err("Invalid day for the given month");
        }

        Ok(())
    }

    /// Validate birth year
    fn validate_birth_year(year: i32) -> Result<(), &'static str> {
        let current_year = chrono::Utc::now().year();

        if year < 1900 {
            return Err("Birth year must be 1900 or later");
        }

        if year > current_year {
            return Err("Birth year cannot be in the future");
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_birthday_date() {
        assert!(BirthdayService::validate_birthday_date(1, 15).is_ok());
        assert!(BirthdayService::validate_birthday_date(2, 29).is_ok());
        assert!(BirthdayService::validate_birthday_date(12, 31).is_ok());

        assert!(BirthdayService::validate_birthday_date(0, 15).is_err());
        assert!(BirthdayService::validate_birthday_date(13, 15).is_err());
        assert!(BirthdayService::validate_birthday_date(2, 30).is_err());
        assert!(BirthdayService::validate_birthday_date(4, 31).is_err());
    }

    #[test]
    fn test_validate_birth_year() {
        assert!(BirthdayService::validate_birth_year(2000).is_ok());
        assert!(BirthdayService::validate_birth_year(1900).is_ok());

        assert!(BirthdayService::validate_birth_year(1899).is_err());
        assert!(BirthdayService::validate_birth_year(2030).is_err());
    }

    #[test]
    fn test_user_birthday_age_calculation() {
        let birthday = UserBirthday {
            user_id: UserId::new(123),
            month: 5,
            day: 15,
            year: Some(2000),
        };

        assert_eq!(birthday.age_on_date(2025), Some(25));
        assert_eq!(birthday.age_on_date(2000), Some(0));
    }

    #[test]
    fn test_user_birthday_formatted_date() {
        let with_year = UserBirthday {
            user_id: UserId::new(123),
            month: 5,
            day: 15,
            year: Some(2000),
        };
        assert_eq!(with_year.formatted_date(), "15/05/2000");

        let without_year = UserBirthday {
            user_id: UserId::new(123),
            month: 5,
            day: 15,
            year: None,
        };
        assert_eq!(without_year.formatted_date(), "15/05");
    }
}
