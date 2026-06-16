use std::{
    collections::{HashMap, HashSet},
    hash::Hash,
};

pub struct MapBuilder<K, VBase, VBuilder>
where
    K: Eq + Hash,
{
    base: HashMap<K, VBase>,
    dirty: HashMap<K, VBuilder>,
    deleted: HashSet<K>,

    to_builder: fn(VBase) -> VBuilder,
    build: fn(VBuilder) -> VBase,
}

pub fn new_map_builder<K, VBase, VBuilder>(
    base: HashMap<K, VBase>,
    to_builder: fn(VBase) -> VBuilder,
    build: fn(VBuilder) -> VBase,
) -> MapBuilder<K, VBase, VBuilder>
where
    K: Eq + Hash,
{
    MapBuilder {
        base,
        dirty: HashMap::new(),
        deleted: HashSet::new(),
        to_builder,
        build,
    }
}

impl<K, VBase, VBuilder> MapBuilder<K, VBase, VBuilder>
where
    K: Eq + Hash + Clone,
    VBase: Clone,
    VBuilder: Clone,
{
    pub fn set(&mut self, key: K, value: VBuilder) {
        self.dirty.insert(key.clone(), value);
        self.deleted.remove(&key);
    }

    pub fn delete(&mut self, key: K) {
        self.deleted.insert(key.clone());
        self.dirty.remove(&key);
    }

    pub fn clear(&mut self) {
        self.dirty = HashMap::new();
        self.deleted = HashSet::with_capacity(self.base.len());
        for key in self.base.keys() {
            self.deleted.insert(key.clone());
        }
    }

    pub fn has(&self, key: &K) -> bool {
        if self.deleted.contains(key) {
            return false;
        }
        if self.dirty.contains_key(key) {
            return true;
        }
        self.base.contains_key(key)
    }

    pub fn build(&self) -> HashMap<K, VBase> {
        if self.dirty.is_empty() && self.deleted.is_empty() {
            return self.base.clone();
        }
        let mut result = self.base.clone();
        for key in &self.deleted {
            result.remove(key);
        }
        for (key, value) in &self.dirty {
            result.insert(key.clone(), (self.build)(value.clone()));
        }
        result
    }

    pub fn to_builder(&self, value: VBase) -> VBuilder {
        (self.to_builder)(value)
    }
}
