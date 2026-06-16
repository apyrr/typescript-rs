use std::{
    hash::Hash,
    sync::{Arc, RwLock},
};

use crate::{FastHashMap as HashMap, FastHashMapExt};

pub struct SyncMap<K, V> {
    m: Arc<RwLock<HashMap<K, Option<V>>>>,
}

impl<K, V> Default for SyncMap<K, V> {
    fn default() -> Self {
        Self {
            m: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl<K, V> SyncMap<K, V> {
    pub fn new() -> Self {
        Self::default()
    }
}

impl<K, V> Clone for SyncMap<K, V>
where
    K: Eq + Hash + Clone,
    V: Clone,
{
    fn clone(&self) -> Self {
        Self { m: self.m.clone() }
    }
}

impl<K, V> SyncMap<K, V>
where
    K: Eq + Hash + Clone,
    V: Clone,
{
    pub fn load(&self, key: &K) -> (Option<V>, bool) {
        let map = self.m.read().unwrap_or_else(|err| err.into_inner());
        match map.get(key) {
            Some(value) => (value.clone(), true),
            None => (None, false),
        }
    }

    pub fn store(&self, key: K, value: Option<V>) {
        self.m
            .write()
            .unwrap_or_else(|err| err.into_inner())
            .insert(key, value);
    }

    pub fn load_or_store(&self, key: K, value: Option<V>) -> (Option<V>, bool) {
        let mut map = self.m.write().unwrap_or_else(|err| err.into_inner());
        if let Some(actual) = map.get(&key) {
            return (actual.clone(), true);
        }
        map.insert(key, value.clone());
        (value, false)
    }

    pub fn delete(&self, key: &K) {
        self.m
            .write()
            .unwrap_or_else(|err| err.into_inner())
            .remove(key);
    }

    pub fn clear(&self) {
        self.m
            .write()
            .unwrap_or_else(|err| err.into_inner())
            .clear();
    }

    pub fn range(&self, mut f: impl FnMut(&K, Option<&V>) -> bool) {
        let map = self.m.read().unwrap_or_else(|err| err.into_inner());
        for (key, value) in map.iter() {
            if !f(key, value.as_ref()) {
                break;
            }
        }
    }

    // Size returns the approximate number of items in the map.
    // Note that this is not a precise count, as the map may be modified
    // concurrently while this method is running.
    pub fn size(&self) -> usize {
        self.m.read().unwrap_or_else(|err| err.into_inner()).len()
    }

    pub fn to_map(&self) -> HashMap<K, Option<V>> {
        self.m.read().unwrap_or_else(|err| err.into_inner()).clone()
    }

    pub fn keys(&self) -> Vec<K> {
        self.m
            .read()
            .unwrap_or_else(|err| err.into_inner())
            .keys()
            .cloned()
            .collect()
    }

    pub fn clone_map(&self) -> Arc<SyncMap<K, V>> {
        Arc::new(SyncMap {
            m: Arc::new(RwLock::new(self.to_map())),
        })
    }
}
