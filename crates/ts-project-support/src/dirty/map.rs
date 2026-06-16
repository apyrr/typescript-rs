use std::{
    collections::HashMap,
    hash::Hash,
    sync::{Arc, Mutex},
};

use super::{Cloneable, entry::MapEntry as BaseMapEntry};

pub struct MapEntry<K, V>
where
    K: Default + Eq + Hash,
    V: Default,
{
    dirty: Arc<Mutex<HashMap<K, BaseMapEntry<K, V>>>>,
    map_entry: BaseMapEntry<K, V>,
}

impl<K, V> MapEntry<K, V>
where
    K: Clone + Default + Eq + Hash,
    V: Cloneable<V> + Default,
{
    fn clone_map_entry(entry: &BaseMapEntry<K, V>) -> BaseMapEntry<K, V> {
        BaseMapEntry {
            key: entry.key.clone(),
            original: entry.original.clone_value(),
            value: entry.value.clone_value(),
            dirty: entry.dirty,
            delete: entry.delete,
        }
    }

    fn store(&self) {
        self.dirty
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .insert(
                self.map_entry.key.clone(),
                Self::clone_map_entry(&self.map_entry),
            );
    }

    pub fn key(&self) -> K {
        self.map_entry.key.clone()
    }

    pub fn original(&self) -> V {
        self.map_entry.original.clone_value()
    }

    pub fn value(&self) -> V {
        if self.map_entry.delete {
            return V::default();
        }
        self.map_entry.value.clone_value()
    }

    pub fn dirty(&self) -> bool {
        self.map_entry.dirty
    }

    pub fn change(&mut self, apply: impl FnOnce(&mut V)) {
        if self.map_entry.delete {
            panic!("tried to change a deleted entry");
        }
        if !self.map_entry.dirty {
            self.map_entry.value = self.map_entry.value.clone_value();
            self.map_entry.dirty = true;
            self.store();
        }
        apply(&mut self.map_entry.value);
        self.store();
    }

    pub fn replace(&mut self, new_value: V) {
        if self.map_entry.delete {
            panic!("tried to change a deleted entry");
        }
        if !self.map_entry.dirty {
            self.map_entry.dirty = true;
            self.store();
        }
        self.map_entry.value = new_value.clone_value();
        self.dirty
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .insert(
                self.map_entry.key.clone(),
                BaseMapEntry {
                    key: self.map_entry.key.clone(),
                    original: self.map_entry.original.clone_value(),
                    value: new_value,
                    dirty: self.map_entry.dirty,
                    delete: self.map_entry.delete,
                },
            );
    }

    pub fn change_if(&mut self, cond: impl FnOnce(&V) -> bool, apply: impl FnOnce(&mut V)) -> bool {
        if cond(&self.map_entry.value) {
            self.change(apply);
            return true;
        }
        false
    }

    pub fn delete(&mut self) {
        self.map_entry.delete = true;
        self.store();
    }

    pub fn locked(&self, f: impl FnOnce(&Self)) {
        f(self);
    }
}

pub struct Map<K, V>
where
    K: Default + Eq + Hash,
    V: Default,
{
    base: HashMap<K, V>,
    dirty: Arc<Mutex<HashMap<K, BaseMapEntry<K, V>>>>,
}

impl<K, V> Clone for Map<K, V>
where
    K: Clone + Default + Eq + Hash,
    V: Cloneable<V> + Default,
{
    fn clone(&self) -> Self {
        Self {
            base: self
                .base
                .iter()
                .map(|(key, value)| (key.clone(), value.clone_value()))
                .collect(),
            dirty: Arc::new(Mutex::new(
                self.dirty
                    .lock()
                    .unwrap_or_else(|err| err.into_inner())
                    .iter()
                    .map(|(key, entry)| {
                        (
                            key.clone(),
                            BaseMapEntry {
                                key: entry.key.clone(),
                                original: entry.original.clone_value(),
                                value: entry.value.clone_value(),
                                dirty: entry.dirty,
                                delete: entry.delete,
                            },
                        )
                    })
                    .collect(),
            )),
        }
    }
}

pub fn new_map<K, V>(base: HashMap<K, V>) -> Map<K, V>
where
    K: Default + Eq + Hash,
    V: Default,
{
    Map {
        base,
        dirty: Arc::new(Mutex::new(HashMap::new())),
    }
}

impl<K, V> Map<K, V>
where
    K: Clone + Default + Eq + Hash,
    V: Cloneable<V> + Default,
{
    pub fn get(&mut self, key: K) -> Option<MapEntry<K, V>> {
        if let Some(entry) = self
            .dirty
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .get(&key)
        {
            if entry.delete {
                return None;
            }
            let map_entry = BaseMapEntry {
                key: entry.key.clone(),
                original: entry.original.clone_value(),
                value: entry.value.clone_value(),
                dirty: entry.dirty,
                delete: entry.delete,
            };
            return Some(MapEntry {
                dirty: self.dirty.clone(),
                map_entry,
            });
        }
        let value = self.base.get(&key)?.clone_value();
        Some(MapEntry {
            dirty: self.dirty.clone(),
            map_entry: BaseMapEntry {
                key,
                original: value.clone_value(),
                value,
                dirty: false,
                delete: false,
            },
        })
    }

    // Add sets a new entry in the dirty map without checking if it exists
    // in the base map. The entry added is considered dirty, so it should
    // be a fresh value, mutable until finalized (i.e., it will not be cloned
    // before changing if a change is made). If modifying an entry that may
    // exist in the base map, use `Change` instead.
    pub fn add(&mut self, key: K, value: V) {
        self.dirty
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .insert(
                key.clone(),
                BaseMapEntry {
                    key,
                    original: V::default(),
                    value,
                    dirty: true,
                    delete: false,
                },
            );
    }

    pub fn change(&mut self, key: K, apply: impl FnOnce(&mut V)) {
        if let Some(mut entry) = self.get(key) {
            entry.change(apply);
        } else {
            panic!("tried to change a non-existent entry");
        }
    }

    pub fn try_delete(&mut self, key: K) -> bool {
        if let Some(mut entry) = self.get(key) {
            entry.delete();
            return true;
        }
        false
    }

    pub fn delete(&mut self, key: K) {
        if !self.try_delete(key) {
            panic!("tried to delete a non-existent entry");
        }
    }

    pub fn range(&mut self, mut f: impl FnMut(&mut MapEntry<K, V>) -> bool) {
        let mut seen_in_dirty = HashMap::<K, ()>::new();
        let dirty_keys: Vec<K> = self
            .dirty
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .keys()
            .cloned()
            .collect();
        for key in dirty_keys {
            let entry = {
                let dirty = self.dirty.lock().unwrap_or_else(|err| err.into_inner());
                MapEntry::clone_map_entry(dirty.get(&key).unwrap())
            };
            seen_in_dirty.insert(entry.key.clone(), ());
            if !entry.delete {
                let map_entry = BaseMapEntry {
                    key: entry.key.clone(),
                    original: entry.original.clone_value(),
                    value: entry.value.clone_value(),
                    dirty: entry.dirty,
                    delete: entry.delete,
                };
                let mut wrapped = MapEntry {
                    dirty: self.dirty.clone(),
                    map_entry,
                };
                if !f(&mut wrapped) {
                    return;
                }
            }
        }
        let base_keys: Vec<K> = self.base.keys().cloned().collect();
        for key in base_keys {
            if seen_in_dirty.contains_key(&key) {
                continue;
            }
            let value = self.base.get(&key).unwrap().clone_value();
            let mut wrapped = MapEntry {
                dirty: self.dirty.clone(),
                map_entry: BaseMapEntry {
                    key,
                    original: value.clone_value(),
                    value,
                    dirty: false,
                    delete: false,
                },
            };
            if !f(&mut wrapped) {
                return;
            }
        }
    }

    pub fn range_(&mut self, f: impl FnMut(&mut MapEntry<K, V>) -> bool) {
        self.range(f);
    }

    pub fn clear(&mut self) {
        self.dirty
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .clear();
        self.base = HashMap::new();
    }

    pub fn finalize(&self) -> (HashMap<K, V>, bool) {
        let dirty = self.dirty.lock().unwrap_or_else(|err| err.into_inner());
        if dirty.is_empty() {
            return (
                self.base
                    .iter()
                    .map(|(key, value)| (key.clone(), value.clone_value()))
                    .collect(),
                false,
            );
        }
        let mut result: HashMap<K, V> = self
            .base
            .iter()
            .map(|(key, value)| (key.clone(), value.clone_value()))
            .collect();
        for (key, entry) in dirty.iter() {
            if entry.delete {
                result.remove(key);
            } else {
                result.insert(key.clone(), entry.value.clone_value());
            }
        }
        (result, true)
    }
}
