use std::hash::Hash;

use crate::SyncMap;

pub struct SyncSet<T> {
    m: SyncMap<T, ()>,
}

impl<T> Clone for SyncSet<T>
where
    T: Eq + Hash + Clone,
{
    fn clone(&self) -> Self {
        Self { m: self.m.clone() }
    }
}

impl<T> Default for SyncSet<T> {
    fn default() -> Self {
        Self {
            m: SyncMap::default(),
        }
    }
}

impl<T> SyncSet<T>
where
    T: Eq + Hash + Clone,
{
    pub fn new() -> Self {
        Self::default()
    }

    pub fn has(&self, key: &T) -> bool {
        let (_, ok) = self.m.load(key);
        ok
    }

    pub fn add(&self, key: T) {
        self.add_if_absent(key);
    }

    // AddIfAbsent adds the key to the set if it is not already present
    // using LoadOrStore. It returns true if the key was not already present
    // (opposite of the return value of LoadOrStore).
    pub fn add_if_absent(&self, key: T) -> bool {
        let (_, loaded) = self.m.load_or_store(key, Some(()));
        !loaded
    }

    pub fn delete(&self, key: &T) {
        self.m.delete(key);
    }

    pub fn range(&self, mut f: impl FnMut(&T) -> bool) {
        self.m.range(|key, _| f(key));
    }

    // Size returns the approximate number of items in the map.
    // Note that this is not a precise count, as the map may be modified
    // concurrently while this method is running.
    pub fn size(&self) -> usize {
        let mut count = 0;
        self.m.range(|_, _| {
            count += 1;
            true
        });
        count
    }

    pub fn is_empty(&self) -> bool {
        let mut empty = true;
        self.m.range(|_, _| {
            empty = false;
            false
        });
        empty
    }

    pub fn to_slice(&self) -> Vec<T> {
        let mut arr = Vec::with_capacity(self.m.size());
        self.m.range(|key, _| {
            arr.push(key.clone());
            true
        });
        arr
    }

    pub fn to_vec(&self) -> Vec<T> {
        self.to_slice()
    }

    pub fn keys(&self) -> Vec<T> {
        let mut keys = Vec::new();
        self.m.range(|key, _| {
            keys.push(key.clone());
            true
        });
        keys
    }
}
