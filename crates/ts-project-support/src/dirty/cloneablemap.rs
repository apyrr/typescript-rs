use std::{collections::HashMap, hash::Hash};

pub type CloneableMap<K, V> = HashMap<K, V>;

pub fn cloneable_map_clone<K, V>(m: &CloneableMap<K, V>) -> CloneableMap<K, V>
where
    K: Eq + Hash + Clone,
    V: Clone,
{
    m.clone()
}
