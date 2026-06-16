use std::{hash::Hash, rc::Rc};

use crate::{FastHashMap as HashMap, FastHashMapExt};

// CopyOnWriteMap is a map that defers cloning of an inherited backing map
// until the first mutation, and supports nested scopes that share the parent's
// map for reads but get their own clone on write.
//
// The zero value is an empty map ready to use.
pub struct CopyOnWriteMap<K, V> {
    m: Rc<HashMap<K, V>>,
    owned: bool,
}

pub struct CopyOnWriteMapScope<K, V> {
    saved: CopyOnWriteMap<K, V>,
}

impl<K, V> CopyOnWriteMapScope<K, V> {
    pub fn restore(self, map: &mut CopyOnWriteMap<K, V>) {
        *map = self.saved;
    }
}

impl<K, V> Default for CopyOnWriteMap<K, V> {
    fn default() -> Self {
        Self {
            m: Rc::new(HashMap::new()),
            owned: false,
        }
    }
}

impl<K, V> CopyOnWriteMap<K, V>
where
    K: Eq + Hash + Clone,
    V: Clone,
{
    // Get returns the value for k and whether it was present.
    pub fn get(&self, k: &K) -> (Option<&V>, bool) {
        match self.m.get(k) {
            Some(v) => (Some(v), true),
            None => (None, false),
        }
    }

    // Has reports whether k is in the map.
    pub fn has(&self, k: &K) -> bool {
        self.m.contains_key(k)
    }

    // Set assigns v to k, cloning the inherited backing map first if necessary.
    pub fn set(&mut self, k: K, v: V) {
        self.ensure_owned();
        Rc::make_mut(&mut self.m).insert(k, v);
    }

    fn ensure_owned(&mut self) {
        if self.owned {
            return;
        }
        Rc::make_mut(&mut self.m);
        self.owned = true;
    }

    // EnterScope returns a token that restores this map to its current state.
    // While the scope is active, the map shares its current backing storage with
    // the parent scope: reads see the inherited entries, and the first mutation
    // transparently clones the storage so the parent's view is not modified.
    pub fn enter_scope(&mut self) -> CopyOnWriteMapScope<K, V> {
        let saved = CopyOnWriteMap {
            m: Rc::clone(&self.m),
            owned: self.owned,
        };
        self.owned = false;
        CopyOnWriteMapScope { saved }
    }
}

pub struct CopyOnWriteSet<K> {
    m: CopyOnWriteMap<K, ()>,
}

pub struct CopyOnWriteSetScope<K> {
    saved: CopyOnWriteMapScope<K, ()>,
}

impl<K> CopyOnWriteSetScope<K> {
    pub fn restore(self, set: &mut CopyOnWriteSet<K>) {
        self.saved.restore(&mut set.m);
    }
}

impl<K> Default for CopyOnWriteSet<K> {
    fn default() -> Self {
        Self {
            m: CopyOnWriteMap::default(),
        }
    }
}

impl<K> CopyOnWriteSet<K>
where
    K: Eq + Hash + Clone,
{
    // Has reports whether k is in the set.
    pub fn has(&self, k: &K) -> bool {
        self.m.get(k).1
    }

    // Set adds k to the set, cloning the inherited backing map first if necessary.
    pub fn add(&mut self, k: K) {
        self.m.set(k, ());
    }

    // EnterScope returns a token that restores this set to its current state.
    // While the scope is active, the set shares its current backing storage with
    // the parent scope: reads see the inherited entries, and the first mutation
    // transparently clones the storage so the parent's view is not modified.
    pub fn enter_scope(&mut self) -> CopyOnWriteSetScope<K> {
        CopyOnWriteSetScope {
            saved: self.m.enter_scope(),
        }
    }
}
