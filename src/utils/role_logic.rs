/// Pure functions for birthday role logic (Discord-agnostic)
use std::collections::HashSet;

/// Represents an action to take on a user's roles
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RoleAction {
    Add,
    Remove,
    NoAction,
}

/// Determine what role action should be taken for a user
pub fn determine_role_action(has_birthday_today: bool, has_role: bool) -> RoleAction {
    match (has_birthday_today, has_role) {
        (true, false) => RoleAction::Add,
        (false, true) => RoleAction::Remove,
        _ => RoleAction::NoAction,
    }
}

/// Calculate which users need role additions and removals
pub fn calculate_role_changes<T: Clone + Eq + std::hash::Hash>(
    birthday_users: &HashSet<T>,
    users_with_role: &HashSet<T>,
) -> (Vec<T>, Vec<T>) {
    let to_add: Vec<T> = birthday_users
        .difference(users_with_role)
        .cloned()
        .collect();
    
    let to_remove: Vec<T> = users_with_role
        .difference(birthday_users)
        .cloned()
        .collect();
    
    (to_add, to_remove)
}

/// Filter a list of items based on a predicate (functional helper)
pub fn filter_items<T, F>(items: Vec<T>, predicate: F) -> Vec<T>
where
    F: Fn(&T) -> bool,
{
    items.into_iter().filter(predicate).collect()
}

/// Check if any role changes are needed
pub fn has_role_changes<T>(to_add: &[T], to_remove: &[T]) -> bool {
    !to_add.is_empty() || !to_remove.is_empty()
}

/// Count total role changes needed
pub fn count_role_changes<T>(to_add: &[T], to_remove: &[T]) -> usize {
    to_add.len() + to_remove.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_determine_role_action() {
        // Should add role
        assert_eq!(
            determine_role_action(true, false),
            RoleAction::Add
        );

        // Should remove role
        assert_eq!(
            determine_role_action(false, true),
            RoleAction::Remove
        );

        // No action needed (has birthday and has role)
        assert_eq!(
            determine_role_action(true, true),
            RoleAction::NoAction
        );

        // No action needed (no birthday and no role)
        assert_eq!(
            determine_role_action(false, false),
            RoleAction::NoAction
        );
    }

    #[test]
    fn test_calculate_role_changes_add_only() {
        let birthday_users: HashSet<u64> = [1, 2, 3].iter().copied().collect();
        let users_with_role: HashSet<u64> = HashSet::new();

        let (to_add, to_remove) = calculate_role_changes(&birthday_users, &users_with_role);

        assert_eq!(to_add.len(), 3);
        assert!(to_add.contains(&1));
        assert!(to_add.contains(&2));
        assert!(to_add.contains(&3));
        assert_eq!(to_remove.len(), 0);
    }

    #[test]
    fn test_calculate_role_changes_remove_only() {
        let birthday_users: HashSet<u64> = HashSet::new();
        let users_with_role: HashSet<u64> = [1, 2, 3].iter().copied().collect();

        let (to_add, to_remove) = calculate_role_changes(&birthday_users, &users_with_role);

        assert_eq!(to_add.len(), 0);
        assert_eq!(to_remove.len(), 3);
        assert!(to_remove.contains(&1));
        assert!(to_remove.contains(&2));
        assert!(to_remove.contains(&3));
    }

    #[test]
    fn test_calculate_role_changes_mixed() {
        let birthday_users: HashSet<u64> = [1, 2, 3, 4].iter().copied().collect();
        let users_with_role: HashSet<u64> = [3, 4, 5, 6].iter().copied().collect();

        let (to_add, to_remove) = calculate_role_changes(&birthday_users, &users_with_role);

        assert_eq!(to_add.len(), 2);
        assert!(to_add.contains(&1));
        assert!(to_add.contains(&2));

        assert_eq!(to_remove.len(), 2);
        assert!(to_remove.contains(&5));
        assert!(to_remove.contains(&6));
    }

    #[test]
    fn test_calculate_role_changes_no_changes() {
        let birthday_users: HashSet<u64> = [1, 2, 3].iter().copied().collect();
        let users_with_role: HashSet<u64> = [1, 2, 3].iter().copied().collect();

        let (to_add, to_remove) = calculate_role_changes(&birthday_users, &users_with_role);

        assert_eq!(to_add.len(), 0);
        assert_eq!(to_remove.len(), 0);
    }

    #[test]
    fn test_filter_items() {
        let numbers = vec![1, 2, 3, 4, 5, 6];
        let evens = filter_items(numbers, |n| n % 2 == 0);
        
        assert_eq!(evens, vec![2, 4, 6]);
    }

    #[test]
    fn test_has_role_changes() {
        let to_add = vec![1, 2];
        let to_remove = vec![3];
        assert!(has_role_changes(&to_add, &to_remove));

        let empty_add: Vec<i32> = vec![];
        let empty_remove: Vec<i32> = vec![];
        assert!(!has_role_changes(&empty_add, &empty_remove));

        assert!(has_role_changes(&to_add, &empty_remove));
        assert!(has_role_changes(&empty_add, &to_remove));
    }

    #[test]
    fn test_count_role_changes() {
        let to_add = vec![1, 2, 3];
        let to_remove = vec![4, 5];
        assert_eq!(count_role_changes(&to_add, &to_remove), 5);

        let empty: Vec<i32> = vec![];
        assert_eq!(count_role_changes(&empty, &empty), 0);
        assert_eq!(count_role_changes(&to_add, &empty), 3);
        assert_eq!(count_role_changes(&empty, &to_remove), 2);
    }
}
