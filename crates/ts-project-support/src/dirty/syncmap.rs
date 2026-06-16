use std::{
    cell::RefCell,
    collections::HashMap,
    hash::Hash,
    sync::{Arc, Mutex, Weak},
};

use ts_collections as collections;

use super::entry::MapEntry as BaseMapEntry;

pub(crate) struct LockedEntry<'a, K, V>
where
    K: Default + Eq + Hash + Clone,
    V: Default,
{
    e: RefCell<&'a mut SyncMapEntry<K, V>>,
}

impl<K, V> LockedEntry<'_, K, V>
where
    K: Clone + Default + Eq + Hash,
    V: Clone + Default,
{
    pub(crate) fn value(&self) -> V {
        self.e.borrow().value_locked()
    }

    pub(crate) fn original(&self) -> V {
        self.e.borrow().map_entry.original.clone()
    }

    pub(crate) fn dirty(&self) -> bool {
        self.e.borrow().map_entry.dirty
    }

    pub(crate) fn change(&self, apply: impl FnOnce(&mut V)) {
        self.e.borrow_mut().change_locked(apply);
    }

    pub(crate) fn change_if(
        &self,
        cond: impl FnOnce(&V) -> bool,
        apply: impl FnOnce(&mut V),
    ) -> bool {
        let mut entry = self.e.borrow_mut();
        if cond(&entry.value_locked()) {
            entry.change_locked(apply);
            return true;
        }
        false
    }

    pub(crate) fn delete(&self) {
        self.e.borrow_mut().delete_locked();
    }

    pub(crate) fn locked(&self, f: impl FnOnce(&Self)) {
        f(self);
    }
}

pub struct SyncMapEntry<K, V>
where
    K: Default + Eq + Hash + Clone,
    V: Default,
{
    owner: Weak<SyncMapInner<K, V>>,
    map_entry: BaseMapEntry<K, V>,
    // proxyFor is set when this entry loses a race to become the dirty entry
    // for a value. Since two goroutines hold a reference to two entries that
    // may try to mutate the same underlying value, all mutations are routed
    // through the one that actually exists in the dirty map.
    pub(crate) proxy_for: Option<Arc<Mutex<SyncMapEntry<K, V>>>>,
    self_ref: Option<Weak<Mutex<SyncMapEntry<K, V>>>>,
}

pub trait SyncMapEntryHandle<K, V>
where
    K: Default + Eq + Hash + Clone,
    V: Default,
{
    fn key(&self) -> K;
    fn value(&self) -> V;
    fn change(&self, apply: impl FnOnce(&mut V));
    fn change_if(&self, cond: impl FnOnce(&V) -> bool, apply: impl FnOnce(&mut V)) -> bool;
    fn delete(&self);
    fn delete_if(&self, cond: impl FnOnce(&V) -> bool) -> bool;
}

impl<K, V> SyncMapEntryHandle<K, V> for Arc<Mutex<SyncMapEntry<K, V>>>
where
    K: Clone + Default + Eq + Hash,
    V: Clone + Default,
{
    fn key(&self) -> K {
        self.lock().unwrap_or_else(|err| err.into_inner()).key()
    }

    fn value(&self) -> V {
        self.lock().unwrap_or_else(|err| err.into_inner()).value()
    }

    fn change(&self, apply: impl FnOnce(&mut V)) {
        self.lock()
            .unwrap_or_else(|err| err.into_inner())
            .change(apply);
    }

    fn change_if(&self, cond: impl FnOnce(&V) -> bool, apply: impl FnOnce(&mut V)) -> bool {
        self.lock()
            .unwrap_or_else(|err| err.into_inner())
            .change_if(cond, apply)
    }

    fn delete(&self) {
        self.lock().unwrap_or_else(|err| err.into_inner()).delete();
    }

    fn delete_if(&self, cond: impl FnOnce(&V) -> bool) -> bool {
        let mut entry = self.lock().unwrap_or_else(|err| err.into_inner());
        if cond(&entry.value()) {
            entry.delete();
            return true;
        }
        false
    }
}

impl<K, V> SyncMapEntry<K, V>
where
    K: Clone + Default + Eq + Hash,
    V: Clone + Default,
{
    fn self_arc(&self) -> Option<Arc<Mutex<SyncMapEntry<K, V>>>> {
        self.self_ref.as_ref().and_then(Weak::upgrade)
    }

    fn is_self_arc(&self, entry: &Arc<Mutex<SyncMapEntry<K, V>>>) -> bool {
        self.self_arc()
            .as_ref()
            .is_some_and(|self_arc| Arc::ptr_eq(self_arc, entry))
    }

    fn owner(&self) -> Arc<SyncMapInner<K, V>> {
        self.owner
            .upgrade()
            .expect("dirty sync map entry outlived its owning map")
    }

    pub fn key(&self) -> K {
        self.map_entry.key.clone()
    }

    pub fn value(&self) -> V {
        if let Some(proxy_for) = &self.proxy_for {
            return proxy_for
                .lock()
                .unwrap_or_else(|err| err.into_inner())
                .value();
        }
        self.value_locked()
    }

    pub fn original(&self) -> V {
        self.map_entry.original.clone()
    }

    pub fn dirty(&self) -> bool {
        if let Some(proxy_for) = &self.proxy_for {
            return proxy_for
                .lock()
                .unwrap_or_else(|err| err.into_inner())
                .dirty();
        }
        self.map_entry.dirty
    }

    pub(crate) fn locked(&mut self, f: impl FnOnce(&LockedEntry<'_, K, V>)) {
        if let Some(proxy_for) = &self.proxy_for {
            proxy_for
                .lock()
                .unwrap_or_else(|err| err.into_inner())
                .locked(f);
            return;
        }
        f(&LockedEntry {
            e: RefCell::new(self),
        });
    }

    pub fn change(&mut self, apply: impl FnOnce(&mut V)) {
        if let Some(proxy_for) = &self.proxy_for {
            proxy_for
                .lock()
                .unwrap_or_else(|err| err.into_inner())
                .change(apply);
            return;
        }
        self.change_locked(apply);
    }

    fn value_locked(&self) -> V {
        if self.map_entry.delete {
            return V::default();
        }
        self.map_entry.value.clone()
    }

    fn change_locked(&mut self, apply: impl FnOnce(&mut V)) {
        if self.map_entry.dirty {
            apply(&mut self.map_entry.value);
            return;
        }

        let stored = self
            .self_arc()
            .unwrap_or_else(|| Arc::new(Mutex::new(self.clone_entry())));
        let (entry_arc, _) = self
            .owner()
            .dirty
            .load_or_store(self.map_entry.key.clone(), Some(stored));
        let entry_arc = entry_arc.unwrap();
        if self.is_self_arc(&entry_arc) {
            self.map_entry.value = self.map_entry.value.clone();
            self.map_entry.dirty = true;
            apply(&mut self.map_entry.value);
            return;
        }

        let mut entry = entry_arc.lock().unwrap_or_else(|err| err.into_inner());
        if !entry.map_entry.dirty {
            entry.map_entry.value = entry.map_entry.value.clone();
            entry.map_entry.dirty = true;
        }
        self.proxy_for = Some(entry_arc.clone());
        self.map_entry.value = entry.map_entry.value.clone();
        self.map_entry.dirty = true;
        self.map_entry.delete = entry.map_entry.delete;
        apply(&mut entry.map_entry.value);
    }

    pub fn change_if(&mut self, cond: impl FnOnce(&V) -> bool, apply: impl FnOnce(&mut V)) -> bool {
        if let Some(proxy_for) = &self.proxy_for {
            return proxy_for
                .lock()
                .unwrap_or_else(|err| err.into_inner())
                .change_if(cond, apply);
        }

        if cond(&self.map_entry.value) {
            self.change_locked(apply);
            return true;
        }
        false
    }

    pub fn delete(&mut self) {
        if let Some(proxy_for) = &self.proxy_for {
            proxy_for
                .lock()
                .unwrap_or_else(|err| err.into_inner())
                .delete();
            return;
        }

        self.delete_locked();
    }

    fn delete_locked(&mut self) {
        if self.map_entry.dirty {
            self.map_entry.delete = true;
            return;
        }

        let stored = self
            .self_arc()
            .unwrap_or_else(|| Arc::new(Mutex::new(self.clone_entry())));
        let (entry_arc, _) = self
            .owner()
            .dirty
            .load_or_store(self.map_entry.key.clone(), Some(stored));
        let entry_arc = entry_arc.unwrap();
        if self.is_self_arc(&entry_arc) {
            self.map_entry.delete = true;
            return;
        }

        let mut entry = entry_arc.lock().unwrap_or_else(|err| err.into_inner());
        self.proxy_for = Some(entry_arc.clone());
        self.map_entry.value = entry.map_entry.value.clone();
        self.map_entry.delete = true;
        self.map_entry.dirty = entry.map_entry.dirty;
        entry.map_entry.delete = true;
    }

    pub fn delete_if(&mut self, cond: impl FnOnce(&V) -> bool) {
        if let Some(proxy_for) = &self.proxy_for {
            proxy_for
                .lock()
                .unwrap_or_else(|err| err.into_inner())
                .delete_if(cond);
            return;
        }
        if cond(&self.map_entry.value) {
            self.delete_locked();
        }
    }

    fn clone_entry(&self) -> SyncMapEntry<K, V> {
        SyncMapEntry {
            owner: self.owner.clone(),
            map_entry: BaseMapEntry {
                key: self.map_entry.key.clone(),
                original: self.map_entry.original.clone(),
                value: self.map_entry.value.clone(),
                dirty: self.map_entry.dirty,
                delete: self.map_entry.delete,
            },
            proxy_for: self.proxy_for.clone(),
            self_ref: None,
        }
    }
}

fn new_sync_map_entry_arc<K, V>(
    owner: &Arc<SyncMapInner<K, V>>,
    map_entry: BaseMapEntry<K, V>,
) -> Arc<Mutex<SyncMapEntry<K, V>>>
where
    K: Default + Eq + Hash + Clone,
    V: Default,
{
    let entry = Arc::new(Mutex::new(SyncMapEntry {
        owner: Arc::downgrade(owner),
        map_entry,
        proxy_for: None,
        self_ref: None,
    }));
    entry.lock().unwrap_or_else(|err| err.into_inner()).self_ref = Some(Arc::downgrade(&entry));
    entry
}

struct SyncMapInner<K, V>
where
    K: Default + Eq + Hash + Clone,
    V: Default,
{
    base: HashMap<K, V>,
    dirty: collections::SyncMap<K, Arc<Mutex<SyncMapEntry<K, V>>>>,
}

pub struct SyncMap<K, V>
where
    K: Default + Eq + Hash + Clone,
    V: Default,
{
    inner: Arc<SyncMapInner<K, V>>,
}

impl<K, V> Clone for SyncMap<K, V>
where
    K: Clone + Default + Eq + Hash,
    V: Clone + Default,
{
    fn clone(&self) -> Self {
        Self {
            inner: Arc::new(SyncMapInner {
                base: self.inner.base.clone(),
                dirty: self.inner.dirty.clone(),
            }),
        }
    }
}

pub fn new_sync_map<K, V>(base: HashMap<K, V>) -> SyncMap<K, V>
where
    K: Default + Eq + Hash + Clone,
    V: Default,
{
    SyncMap {
        inner: Arc::new(SyncMapInner {
            base,
            dirty: collections::SyncMap::default(),
        }),
    }
}

impl<K, V> SyncMap<K, V>
where
    K: Clone + Default + Eq + Hash,
    V: Clone + Default,
{
    pub fn new_entry(&self, key: K, value: V) -> Arc<Mutex<SyncMapEntry<K, V>>> {
        new_sync_map_entry_arc(
            &self.inner,
            BaseMapEntry {
                key,
                original: V::default(),
                value,
                dirty: true,
                delete: false,
            },
        )
    }

    pub fn load(&self, key: K) -> Option<Arc<Mutex<SyncMapEntry<K, V>>>> {
        if let (Some(entry), true) = self.inner.dirty.load(&key) {
            let deleted = entry
                .lock()
                .unwrap_or_else(|err| err.into_inner())
                .map_entry
                .delete;
            if deleted {
                return None;
            }
            return Some(entry);
        }
        if let Some(val) = self.inner.base.get(&key) {
            return Some(new_sync_map_entry_arc(
                &self.inner,
                BaseMapEntry {
                    key,
                    original: val.clone(),
                    value: val.clone(),
                    dirty: false,
                    delete: false,
                },
            ));
        }
        None
    }

    pub fn load_or_store(
        &self,
        key: K,
        value: Arc<Mutex<SyncMapEntry<K, V>>>,
    ) -> (Arc<Mutex<SyncMapEntry<K, V>>>, bool) {
        // Check for existence in the base map first so the sync map access is atomic.
        if let Some(base_value) = self.inner.base.get(&key) {
            if let (Some(dirty), true) = self.inner.dirty.load(&key) {
                let deleted = dirty
                    .lock()
                    .unwrap_or_else(|err| err.into_inner())
                    .map_entry
                    .delete;
                if deleted {
                    return (dirty, false);
                }
                return (dirty, true);
            }
            return (
                new_sync_map_entry_arc(
                    &self.inner,
                    BaseMapEntry {
                        key,
                        original: base_value.clone(),
                        value: base_value.clone(),
                        dirty: false,
                        delete: false,
                    },
                ),
                true,
            );
        }
        let (entry, loaded) = self.inner.dirty.load_or_store(key, Some(value));
        let entry = entry.unwrap();
        if loaded {
            let deleted = entry
                .lock()
                .unwrap_or_else(|err| err.into_inner())
                .map_entry
                .delete;
            if deleted {
                return (entry, false);
            }
        }
        (entry, loaded)
    }

    pub fn delete(&self, key: K) {
        let current = self.inner.base.get(&key).cloned().unwrap_or_default();
        let stored = new_sync_map_entry_arc(
            &self.inner,
            BaseMapEntry {
                key: key.clone(),
                original: current,
                value: V::default(),
                dirty: false,
                delete: true,
            },
        );
        let (entry, loaded) = self.inner.dirty.load_or_store(key, Some(stored));
        if loaded {
            entry
                .unwrap()
                .lock()
                .unwrap_or_else(|err| err.into_inner())
                .delete();
        }
    }

    pub fn range_(&self, mut f: impl FnMut(K, &Arc<Mutex<SyncMapEntry<K, V>>>) -> bool) {
        let mut seen_in_dirty = HashMap::<K, ()>::new();
        self.inner.dirty.range(|key, entry| {
            let entry = entry.unwrap();
            seen_in_dirty.insert(key.clone(), ());
            let deleted = entry
                .lock()
                .unwrap_or_else(|err| err.into_inner())
                .map_entry
                .delete;
            if !deleted && !f(key.clone(), entry) {
                return false;
            }
            true
        });
        for (key, value) in &self.inner.base {
            if seen_in_dirty.contains_key(key) {
                continue;
            }
            let entry = new_sync_map_entry_arc(
                &self.inner,
                BaseMapEntry {
                    key: key.clone(),
                    original: value.clone(),
                    value: value.clone(),
                    dirty: false,
                    delete: false,
                },
            );
            if !f(key.clone(), &entry) {
                break;
            }
        }
    }

    fn finalize_internal(&self, hooks: FinalizationHooks<K, V>) -> (HashMap<K, V>, bool) {
        let mut changed = false;
        let mut result = self.inner.base.clone();

        self.inner.dirty.range(|key, entry| {
            let entry = entry.unwrap();
            let entry = entry.lock().unwrap_or_else(|err| err.into_inner());
            if entry.map_entry.delete {
                if !changed {
                    result = if self.inner.base.is_empty() {
                        HashMap::new()
                    } else {
                        self.inner.base.clone()
                    };
                    changed = true;
                }
                if let Some(on_delete) = &hooks.on_delete {
                    on_delete(key.clone(), entry.map_entry.value.clone());
                }
                result.remove(key);
            } else if entry.map_entry.dirty {
                if !changed {
                    result = if self.inner.base.is_empty() {
                        HashMap::new()
                    } else {
                        self.inner.base.clone()
                    };
                    changed = true;
                }
                if self.inner.base.contains_key(key) {
                    if let Some(on_change) = &hooks.on_change {
                        on_change(
                            key.clone(),
                            entry.map_entry.original.clone(),
                            entry.map_entry.value.clone(),
                        );
                    }
                } else if let Some(on_add) = &hooks.on_add {
                    on_add(key.clone(), entry.map_entry.value.clone());
                }
                result.insert(key.clone(), entry.map_entry.value.clone());
            }
            true
        });
        (result, changed)
    }

    pub fn finalize(&self) -> (HashMap<K, V>, bool) {
        self.finalize_internal(FinalizationHooks::default())
    }

    pub fn finalize_with(&self, hooks: FinalizationHooks<K, V>) -> (HashMap<K, V>, bool) {
        self.finalize_internal(hooks)
    }
}

#[derive(Default)]
pub struct FinalizationHooks<K, V> {
    pub on_delete: Option<Box<dyn Fn(K, V) + Send + Sync>>,
    pub on_change: Option<Box<dyn Fn(K, V, V) + Send + Sync>>,
    pub on_add: Option<Box<dyn Fn(K, V) + Send + Sync>>,
}
