/// Tests for utility functions
#[cfg(test)]
mod tests {
    use crate::utils::datetime::get_month_name;

    #[test]
    fn test_get_month_name_valid() {
        assert_eq!(get_month_name(1), "January");
        assert_eq!(get_month_name(6), "June");
        assert_eq!(get_month_name(12), "December");
    }

    #[test]
    fn test_get_month_name_invalid() {
        assert_eq!(get_month_name(0), "Unknown");
        assert_eq!(get_month_name(13), "Unknown");
        assert_eq!(get_month_name(-1), "Unknown");
        assert_eq!(get_month_name(100), "Unknown");
    }

    #[test]
    fn test_get_month_name_all_months() {
        let expected = [
            "January", "February", "March", "April", "May", "June",
            "July", "August", "September", "October", "November", "December"
        ];
        
        for (i, &expected_name) in expected.iter().enumerate() {
            assert_eq!(get_month_name((i + 1) as i32), expected_name);
        }
    }
}
