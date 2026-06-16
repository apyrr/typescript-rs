use std::hash::Hash;

use crate::{FastHashMap as HashMap, FastHashMapExt};

#[derive(Clone, Debug)]
pub struct MultiMap<K, V> {
    pub m: HashMap<K, Vec<V>>,
}

impl<K, V> Default for MultiMap<K, V> {
    fn default() -> Self {
        Self { m: HashMap::new() }
    }
}

impl<K, V> MultiMap<K, V> {
    pub fn new() -> Self {
        Self::default()
    }
}

pub fn new_multi_map_with_size_hint<K, V>(hint: usize) -> MultiMap<K, V> {
    MultiMap {
        m: HashMap::with_capacity(hint),
    }
}

pub fn group_by<K, V>(items: &[V], group_id: impl Fn(&V) -> K) -> MultiMap<K, V>
where
    K: Eq + Hash,
    V: Clone + Eq,
{
    let mut m = MultiMap::default();
    for item in items {
        m.add(group_id(item), item.clone());
    }
    m
}

impl<K, V> MultiMap<K, V>
where
    K: Eq + Hash,
{
    pub fn has(&self, key: &K) -> bool {
        self.m.contains_key(key)
    }

    pub fn get(&self, key: &K) -> Option<&[V]> {
        self.m.get(key).map(Vec::as_slice)
    }

    pub fn add(&mut self, key: K, value: V) {
        self.m.entry(key).or_default().push(value);
    }
}

impl<K, V> MultiMap<K, V>
where
    K: Eq + Hash,
    V: Eq,
{
    pub fn remove(&mut self, key: &K, value: &V) {
        if let Some(values) = self.m.get_mut(key)
            && let Some(i) = values.iter().position(|existing| existing == value)
        {
            if values.len() == 1 {
                self.m.remove(key);
            } else {
                values.remove(i);
            }
        }
    }

    pub fn remove_all(&mut self, key: &K) {
        self.m.remove(key);
    }

    pub fn len(&self) -> usize {
        self.m.len()
    }

    pub fn is_empty(&self) -> bool {
        self.m.is_empty()
    }

    pub fn keys(&self) -> impl Iterator<Item = &K> {
        self.m.keys()
    }

    pub fn values(&self) -> impl Iterator<Item = &[V]> {
        self.m.values().map(Vec::as_slice)
    }

    pub fn clear(&mut self) {
        self.m.clear();
    }
}
