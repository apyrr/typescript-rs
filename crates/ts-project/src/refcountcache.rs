use std::{
    collections::HashMap,
    fmt,
    hash::Hash,
    sync::{
        Arc, Mutex,
        atomic::{AtomicI32, Ordering},
    },
};

#[derive(Clone)]
pub(crate) struct RefCount {
    value: Arc<AtomicI32>,
}

impl RefCount {
    fn new(value: i32) -> Self {
        Self {
            value: Arc::new(AtomicI32::new(value)),
        }
    }

    fn get(&self) -> i32 {
        self.value.load(Ordering::SeqCst)
    }

    fn increment(&self) {
        self.value.fetch_add(1, Ordering::SeqCst);
    }

    fn decrement(&self) -> i32 {
        self.value.fetch_sub(1, Ordering::SeqCst) - 1
    }
}

impl fmt::Debug for RefCount {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.get().fmt(f)
    }
}

impl PartialEq<i32> for RefCount {
    fn eq(&self, other: &i32) -> bool {
        self.get() == *other
    }
}

pub(crate) struct RefCountCacheEntry<V> {
    pub(crate) value: V,
    pub(crate) ref_count: RefCount,
}

impl<V: Clone> Clone for RefCountCacheEntry<V> {
    fn clone(&self) -> Self {
        Self {
            value: self.value.clone(),
            ref_count: self.ref_count.clone(),
        }
    }
}

pub(crate) struct RefCountCacheEntries<K, V>
where
    K: Eq + Hash,
{
    inner: Mutex<HashMap<K, Arc<Mutex<RefCountCacheEntry<V>>>>>,
}

impl<K, V> RefCountCacheEntries<K, V>
where
    K: Eq + Hash,
{
    fn new() -> Self {
        Self {
            inner: Mutex::new(HashMap::new()),
        }
    }

    fn from_inner(inner: HashMap<K, Arc<Mutex<RefCountCacheEntry<V>>>>) -> Self {
        Self {
            inner: Mutex::new(inner),
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<K, Arc<Mutex<RefCountCacheEntry<V>>>>> {
        self.inner.lock().unwrap_or_else(|err| err.into_inner())
    }
}

impl<K, V> RefCountCacheEntries<K, V>
where
    K: Eq + Hash + Clone,
    V: Clone,
{
    pub(crate) fn load(&self, key: &K) -> Option<RefCountCacheEntry<V>> {
        self.lock()
            .get(key)
            .map(|entry| entry.lock().unwrap_or_else(|err| err.into_inner()).clone())
    }

    pub(crate) fn range(&self, mut f: impl FnMut(&K, RefCountCacheEntry<V>) -> bool) {
        let entries = self.lock();
        for (key, entry) in entries.iter() {
            let entry = entry.lock().unwrap_or_else(|err| err.into_inner()).clone();
            if !f(key, entry) {
                break;
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RefCountCacheOptions {
    // DisableDeletion prevents entries from being removed from the cache.
    // Used for testing.
    pub disable_deletion: bool,
}

pub struct RefCountCache<K, AcquireArgs, V>
where
    K: Eq + Hash,
{
    pub options: RefCountCacheOptions,
    pub(crate) entries: RefCountCacheEntries<K, V>,

    parse: Arc<dyn Fn(&K, AcquireArgs) -> V + Send + Sync>,
}

pub fn new_ref_count_cache<K, AcquireArgs, V>(
    options: RefCountCacheOptions,
    parse: impl Fn(&K, AcquireArgs) -> V + Send + Sync + 'static,
) -> RefCountCache<K, AcquireArgs, V>
where
    K: Eq + Hash,
{
    RefCountCache {
        options,
        entries: RefCountCacheEntries::new(),
        parse: Arc::new(parse),
    }
}

impl<K, AcquireArgs, V> Clone for RefCountCache<K, AcquireArgs, V>
where
    K: Clone + Eq + Hash,
{
    fn clone(&self) -> Self {
        Self {
            options: self.options,
            entries: RefCountCacheEntries::from_inner(self.entries.lock().clone()),
            parse: Arc::clone(&self.parse),
        }
    }
}

impl<K, AcquireArgs, V> RefCountCache<K, AcquireArgs, V>
where
    K: Eq + Hash + Clone,
    V: Clone,
{
    // Acquire retrieves or creates a cache entry for the given identity and hash.
    // If an entry exists with matching identity and hash, its refcount is incremented
    // and the cached value is returned. Otherwise, parse() is called to create the
    // value, which is stored and returned with refcount 1.
    //
    // The caller is responsible for calling Deref when done with the value.
    pub fn acquire(&self, identity: K, acquire_args: AcquireArgs) -> V {
        {
            let entries = self.entries.lock();
            if let Some(existing) = entries.get(&identity).cloned() {
                drop(entries);
                let existing_guard = existing.lock().unwrap_or_else(|err| err.into_inner());
                if existing_guard.ref_count.get() > 0 || self.options.disable_deletion {
                    existing_guard.ref_count.increment();
                    return existing_guard.value.clone();
                }
            }
        }

        let parsed = (self.parse)(&identity, acquire_args);
        let mut entries = self.entries.lock();
        if let Some(existing) = entries.get(&identity).cloned() {
            let existing_guard = existing.lock().unwrap_or_else(|err| err.into_inner());
            if existing_guard.ref_count.get() > 0 || self.options.disable_deletion {
                existing_guard.ref_count.increment();
                return existing_guard.value.clone();
            }
        }

        entries.insert(
            identity,
            Arc::new(Mutex::new(RefCountCacheEntry {
                value: parsed.clone(),
                ref_count: RefCount::new(1),
            })),
        );
        parsed
    }

    pub fn has(&self, identity: &K) -> bool {
        self.entries.lock().contains_key(identity)
    }

    pub fn len(&self) -> usize {
        self.entries.lock().len()
    }

    // Ref increments the reference count for an existing entry.
    // Panics if the entry does not exist.
    pub fn r#ref(&self, identity: K) {
        let entry = {
            let entries = self.entries.lock();
            entries.get(&identity).cloned()
        };
        let Some(entry) = entry else {
            panic!("cache entry not found");
        };
        let entry_guard = entry.lock().unwrap_or_else(|err| err.into_inner());
        if entry_guard.ref_count.get() <= 0 && !self.options.disable_deletion {
            // Entry was deleted while we were acquiring the lock
            let value = entry_guard.value.clone();
            drop(entry_guard);
            self.entries.lock().insert(
                identity,
                Arc::new(Mutex::new(RefCountCacheEntry {
                    value,
                    ref_count: RefCount::new(1),
                })),
            );
            return;
        }
        entry_guard.ref_count.increment();
    }

    // Deref decrements the reference count for an entry.
    // When the refcount reaches zero, the entry is removed from the cache
    // (unless DisableDeletion is set).
    pub fn deref(&self, identity: &K) {
        let entry = {
            let entries = self.entries.lock();
            entries.get(identity).cloned()
        };
        let Some(entry) = entry else {
            return;
        };
        let entry = entry.lock().unwrap_or_else(|err| err.into_inner());
        let ref_count = entry.ref_count.decrement();
        let should_delete = ref_count <= 0 && !self.options.disable_deletion;
        drop(entry);
        if should_delete {
            self.entries.lock().remove(identity);
        }
    }
}
