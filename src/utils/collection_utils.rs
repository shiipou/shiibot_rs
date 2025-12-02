/// Pure collection and list manipulation utilities (Discord-agnostic)
use std::collections::{HashMap, HashSet};

/// Check if a vector is empty
pub fn is_empty<T>(items: &[T]) -> bool {
    items.is_empty()
}

/// Get the length of a vector
pub fn count<T>(items: &[T]) -> usize {
    items.len()
}

/// Get the first element of a slice
pub fn first<T: Clone>(items: &[T]) -> Option<T> {
    items.first().cloned()
}

/// Get the last element of a slice
pub fn last<T: Clone>(items: &[T]) -> Option<T> {
    items.last().cloned()
}

/// Partition a vector into two based on a predicate
pub fn partition<T, F>(items: Vec<T>, predicate: F) -> (Vec<T>, Vec<T>)
where
    F: Fn(&T) -> bool,
{
    let mut true_items = Vec::new();
    let mut false_items = Vec::new();
    
    for item in items {
        if predicate(&item) {
            true_items.push(item);
        } else {
            false_items.push(item);
        }
    }
    
    (true_items, false_items)
}

/// Group items by a key function
pub fn group_by<T, K, F>(items: Vec<T>, key_fn: F) -> HashMap<K, Vec<T>>
where
    K: Eq + std::hash::Hash,
    F: Fn(&T) -> K,
{
    let mut groups: HashMap<K, Vec<T>> = HashMap::new();
    
    for item in items {
        let key = key_fn(&item);
        groups.entry(key).or_default().push(item);
    }
    
    groups
}

/// Find the index of the first element matching a predicate
pub fn find_index<T, F>(items: &[T], predicate: F) -> Option<usize>
where
    F: Fn(&T) -> bool,
{
    items.iter().position(predicate)
}

/// Check if all elements satisfy a predicate
pub fn all<T, F>(items: &[T], predicate: F) -> bool
where
    F: Fn(&T) -> bool,
{
    items.iter().all(predicate)
}

/// Check if any element satisfies a predicate
pub fn any<T, F>(items: &[T], predicate: F) -> bool
where
    F: Fn(&T) -> bool,
{
    items.iter().any(predicate)
}

/// Remove duplicates from a vector (preserving order)
pub fn dedup<T: Eq + std::hash::Hash + Clone>(items: Vec<T>) -> Vec<T> {
    let mut seen = HashSet::new();
    let mut result = Vec::new();
    
    for item in items {
        if seen.insert(item.clone()) {
            result.push(item);
        }
    }
    
    result
}

/// Chunk a vector into smaller vectors of size n
pub fn chunk<T: Clone>(items: &[T], size: usize) -> Vec<Vec<T>> {
    if size == 0 {
        return vec![];
    }
    
    items.chunks(size).map(|chunk| chunk.to_vec()).collect()
}

/// Take the first n elements
pub fn take<T: Clone>(items: &[T], n: usize) -> Vec<T> {
    items.iter().take(n).cloned().collect()
}

/// Skip the first n elements
pub fn skip<T: Clone>(items: &[T], n: usize) -> Vec<T> {
    items.iter().skip(n).cloned().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_empty() {
        assert!(is_empty::<i32>(&[]));
        assert!(!is_empty(&[1, 2, 3]));
    }

    #[test]
    fn test_count() {
        assert_eq!(count::<i32>(&[]), 0);
        assert_eq!(count(&[1, 2, 3, 4]), 4);
    }

    #[test]
    fn test_first() {
        assert_eq!(first(&[1, 2, 3]), Some(1));
        assert_eq!(first::<i32>(&[]), None);
    }

    #[test]
    fn test_last() {
        assert_eq!(last(&[1, 2, 3]), Some(3));
        assert_eq!(last::<i32>(&[]), None);
    }

    #[test]
    fn test_partition() {
        let numbers = vec![1, 2, 3, 4, 5, 6];
        let (evens, odds) = partition(numbers, |n| n % 2 == 0);
        
        assert_eq!(evens, vec![2, 4, 6]);
        assert_eq!(odds, vec![1, 3, 5]);
    }

    #[test]
    fn test_group_by() {
        let items = vec!["apple", "apricot", "banana", "blueberry", "cherry"];
        let groups = group_by(items, |s| s.chars().next().unwrap());
        
        assert_eq!(groups.get(&'a').unwrap(), &vec!["apple", "apricot"]);
        assert_eq!(groups.get(&'b').unwrap(), &vec!["banana", "blueberry"]);
        assert_eq!(groups.get(&'c').unwrap(), &vec!["cherry"]);
    }

    #[test]
    fn test_find_index() {
        let numbers = vec![10, 20, 30, 40];
        
        assert_eq!(find_index(&numbers, |n| *n == 30), Some(2));
        assert_eq!(find_index(&numbers, |n| *n == 99), None);
    }

    #[test]
    fn test_all() {
        assert!(all(&[2, 4, 6, 8], |n| n % 2 == 0));
        assert!(!all(&[2, 3, 4], |n| n % 2 == 0));
    }

    #[test]
    fn test_any() {
        assert!(any(&[1, 2, 3], |n| *n == 2));
        assert!(!any(&[1, 3, 5], |n| *n == 2));
    }

    #[test]
    fn test_dedup() {
        let items = vec![1, 2, 2, 3, 1, 4, 3];
        let unique = dedup(items);
        
        assert_eq!(unique, vec![1, 2, 3, 4]);
    }

    #[test]
    fn test_chunk() {
        let items = vec![1, 2, 3, 4, 5, 6, 7];
        let chunks = chunk(&items, 3);
        
        assert_eq!(chunks, vec![vec![1, 2, 3], vec![4, 5, 6], vec![7]]);
    }

    #[test]
    fn test_chunk_zero_size() {
        let items = vec![1, 2, 3];
        let chunks = chunk(&items, 0);
        
        assert_eq!(chunks.len(), 0);
    }

    #[test]
    fn test_take() {
        let items = vec![1, 2, 3, 4, 5];
        
        assert_eq!(take(&items, 3), vec![1, 2, 3]);
        assert_eq!(take(&items, 10), vec![1, 2, 3, 4, 5]); // More than available
    }

    #[test]
    fn test_skip() {
        let items = vec![1, 2, 3, 4, 5];
        
        assert_eq!(skip(&items, 2), vec![3, 4, 5]);
        assert_eq!(skip(&items, 10), Vec::<i32>::new()); // Skip all
    }
}
