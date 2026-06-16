use std::{collections::HashMap, hash::Hash};

pub fn clone_map_if_nil<K, V, T>(
    dirty: &T,
    original: Option<&T>,
    get_map: impl Fn(&T) -> Option<&HashMap<K, V>>,
) -> HashMap<K, V>
where
    K: Eq + Hash + Clone,
    V: Clone,
{
    let dirty_map = get_map(dirty);
    if dirty_map.is_none() {
        let Some(original) = original else {
            return HashMap::new();
        };
        let Some(original_map) = get_map(original) else {
            return HashMap::new();
        };
        return original_map.clone();
    }
    dirty_map.unwrap().clone()
}
