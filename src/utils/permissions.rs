/// Pure functions for permission and ownership logic (Discord-agnostic)

/// Check if a user ID matches an owner ID
pub fn is_owner(user_id: u64, owner_id: u64) -> bool {
    user_id == owner_id
}

/// Check if a user is in a list of authorized users
pub fn is_authorized(user_id: u64, authorized_users: &[u64]) -> bool {
    authorized_users.contains(&user_id)
}

/// Check if a user has any of the required role IDs
pub fn has_any_role(user_roles: &[u64], required_roles: &[u64]) -> bool {
    user_roles.iter().any(|role| required_roles.contains(role))
}

/// Check if a user has all of the required role IDs
pub fn has_all_roles(user_roles: &[u64], required_roles: &[u64]) -> bool {
    required_roles.iter().all(|role| user_roles.contains(role))
}

/// Filter items by owner
pub fn filter_by_owner<T>(items: Vec<(T, u64)>, owner_id: u64) -> Vec<T> {
    items
        .into_iter()
        .filter(|(_, item_owner)| *item_owner == owner_id)
        .map(|(item, _)| item)
        .collect()
}

/// Count items owned by a specific user
pub fn count_owned_items<T>(items: &[(T, u64)], owner_id: u64) -> usize {
    items.iter().filter(|(_, item_owner)| *item_owner == owner_id).count()
}

/// Check if a list contains duplicates
pub fn has_duplicates<T: Eq + std::hash::Hash>(items: &[T]) -> bool {
    let mut seen = std::collections::HashSet::new();
    !items.iter().all(|item| seen.insert(item))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_owner() {
        assert!(is_owner(123, 123));
        assert!(!is_owner(123, 456));
    }

    #[test]
    fn test_is_authorized() {
        let authorized = vec![100, 200, 300];
        
        assert!(is_authorized(100, &authorized));
        assert!(is_authorized(200, &authorized));
        assert!(!is_authorized(999, &authorized));
    }

    #[test]
    fn test_is_authorized_empty_list() {
        let authorized: Vec<u64> = vec![];
        assert!(!is_authorized(123, &authorized));
    }

    #[test]
    fn test_has_any_role() {
        let user_roles = vec![10, 20, 30];
        let required = vec![20, 40];
        
        assert!(has_any_role(&user_roles, &required)); // Has role 20
        
        let required_none = vec![40, 50];
        assert!(!has_any_role(&user_roles, &required_none));
    }

    #[test]
    fn test_has_any_role_empty() {
        let user_roles = vec![10, 20];
        let required: Vec<u64> = vec![];
        
        assert!(!has_any_role(&user_roles, &required));
    }

    #[test]
    fn test_has_all_roles() {
        let user_roles = vec![10, 20, 30, 40];
        let required = vec![10, 20];
        
        assert!(has_all_roles(&user_roles, &required));
        
        let required_missing = vec![10, 20, 50];
        assert!(!has_all_roles(&user_roles, &required_missing));
    }

    #[test]
    fn test_has_all_roles_empty_required() {
        let user_roles = vec![10, 20];
        let required: Vec<u64> = vec![];
        
        assert!(has_all_roles(&user_roles, &required)); // Vacuous truth
    }

    #[test]
    fn test_filter_by_owner() {
        let items = vec![
            ("item1", 100),
            ("item2", 200),
            ("item3", 100),
            ("item4", 300),
        ];
        
        let owned = filter_by_owner(items, 100);
        assert_eq!(owned, vec!["item1", "item3"]);
    }

    #[test]
    fn test_filter_by_owner_none() {
        let items = vec![
            ("item1", 100),
            ("item2", 200),
        ];
        
        let owned = filter_by_owner(items, 999);
        assert_eq!(owned.len(), 0);
    }

    #[test]
    fn test_count_owned_items() {
        let items = vec![
            ("item1", 100),
            ("item2", 200),
            ("item3", 100),
            ("item4", 100),
        ];
        
        assert_eq!(count_owned_items(&items, 100), 3);
        assert_eq!(count_owned_items(&items, 200), 1);
        assert_eq!(count_owned_items(&items, 999), 0);
    }

    #[test]
    fn test_has_duplicates() {
        assert!(has_duplicates(&[1, 2, 3, 2]));
        assert!(has_duplicates(&[1, 1]));
        
        assert!(!has_duplicates(&[1, 2, 3, 4]));
        assert!(!has_duplicates(&[1]));
        assert!(!has_duplicates::<i32>(&[]));
    }

    #[test]
    fn test_has_duplicates_strings() {
        assert!(has_duplicates(&["a", "b", "a"]));
        assert!(!has_duplicates(&["a", "b", "c"]));
    }
}
