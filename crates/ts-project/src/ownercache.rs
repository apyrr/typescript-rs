use std::{
    collections::{HashMap, HashSet},
    hash::Hash,
    sync::{Arc, Mutex},
};

#[derive(Clone)]
pub(crate) struct OwnerCacheEntry<V> {
    pub(crate) value: V,
    pub(crate) owners: HashSet<u64>,
}

pub(crate) struct OwnerCacheEntries<K, V>
where
    K: Eq + Hash,
{
    inner: Arc<Mutex<HashMap<K, Arc<Mutex<OwnerCacheEntry<V>>>>>>,
}

impl<K, V> OwnerCacheEntries<K, V>
where
    K: Eq + Hash,
{
    fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<K, Arc<Mutex<OwnerCacheEntry<V>>>>> {
        self.inner.lock().unwrap_or_else(|err| err.into_inner())
    }
}

impl<K, V> OwnerCacheEntries<K, V>
where
    K: Eq + Hash + Clone,
    V: Clone,
{
    pub(crate) fn load<Q>(&self, key: &Q) -> Option<OwnerCacheEntry<V>>
    where
        K: std::borrow::Borrow<Q>,
        Q: Eq + Hash + ?Sized,
    {
        self.lock()
            .get(key)
            .map(|entry| entry.lock().unwrap_or_else(|err| err.into_inner()).clone())
    }
}

// OwnerCache is like RefCountCache, but each entry tracks the set of its
// owners instead of a count. We use this to associate extended config cache
// entries with each snapshot that contains them, since the same config can
// be Acquired multiple times during config parsing while only appearing once in
// the ParsedCommandLine's list of extended files. When updating this code, check
// if the same changes should be made to RefCountCache as well.
pub struct OwnerCache<K, V, LoadArgs>
where
    K: Eq + Hash,
{
    pub(crate) entries: OwnerCacheEntries<K, V>,

    is_expired: Option<Arc<dyn Fn(&K, &V, &LoadArgs) -> bool + Send + Sync>>,
}

pub fn new_owner_cache<K, V, LoadArgs>(
    is_expired: Option<impl Fn(&K, &V, &LoadArgs) -> bool + Send + Sync + 'static>,
) -> OwnerCache<K, V, LoadArgs>
where
    K: Eq + Hash,
{
    OwnerCache {
        entries: OwnerCacheEntries::new(),
        is_expired: is_expired.map(|f| Arc::new(f) as _),
    }
}

impl<K, V, LoadArgs> Clone for OwnerCache<K, V, LoadArgs>
where
    K: Clone + Eq + Hash,
{
    fn clone(&self) -> Self {
        Self {
            entries: OwnerCacheEntries {
                inner: Arc::clone(&self.entries.inner),
            },
            is_expired: self.is_expired.clone(),
        }
    }
}

impl<K, V, LoadArgs> OwnerCache<K, V, LoadArgs>
where
    K: Eq + Hash + Clone,
    V: Clone + Default,
{
    pub fn load_and_acquire(
        &self,
        identity: K,
        owner: u64,
        load_args: &LoadArgs,
        parse: impl FnOnce(K, &LoadArgs) -> V,
    ) -> V {
        let (entry, loaded) = self.load_or_store_locked_entry(identity.clone());
        let mut entry = entry.lock().unwrap_or_else(|err| err.into_inner());
        if !loaded
            || self
                .is_expired
                .as_ref()
                .is_some_and(|is_expired| is_expired(&identity, &entry.value, load_args))
        {
            entry.value = parse(identity, load_args);
        }
        entry.owners.insert(owner);
        entry.value.clone()
    }

    pub fn acquire(&self, identity: K, owner: u64, value: V) {
        let (entry, loaded) = self.load_or_store_locked_entry(identity);
        let mut entry = entry.lock().unwrap_or_else(|err| err.into_inner());
        if !loaded {
            entry.value = value;
        }
        entry.owners.insert(owner);
    }

    // AddOwner adds an owner to an existing live entry. The entry must exist
    // and have at least one current owner; callers must ensure the entry is
    // kept alive (e.g. via snapshot ref counting).
    pub fn add_owner(&self, identity: &K, owner: u64) {
        let entry = {
            let entries = self.entries.lock();
            entries.get(identity).cloned()
        };
        let Some(entry) = entry else {
            panic!("OwnerCache.AddOwner: entry not found");
        };
        let mut entry = entry.lock().unwrap_or_else(|err| err.into_inner());
        if entry.owners.is_empty() {
            panic!("OwnerCache.AddOwner: entry has no owners");
        }
        entry.owners.insert(owner);
    }

    pub fn has(&self, identity: &K) -> bool {
        self.entries.lock().contains_key(identity)
    }

    pub fn len(&self) -> usize {
        self.entries.lock().len()
    }

    pub fn release(&self, identity: &K, owner: u64) {
        let entry = {
            let entries = self.entries.lock();
            entries.get(identity).cloned()
        };
        let Some(entry) = entry else {
            return;
        };
        let mut entry = entry.lock().unwrap_or_else(|err| err.into_inner());
        entry.owners.remove(&owner);
        let should_delete = entry.owners.is_empty();
        drop(entry);
        if should_delete {
            self.entries.lock().remove(identity);
        }
    }

    fn load_or_store_locked_entry(&self, key: K) -> (Arc<Mutex<OwnerCacheEntry<V>>>, bool) {
        loop {
            let mut entries = self.entries.lock();
            if let Some(existing) = entries.get(&key).cloned() {
                drop(entries);
                if existing
                    .lock()
                    .unwrap_or_else(|err| err.into_inner())
                    .owners
                    .is_empty()
                {
                    continue;
                }
                return (existing, true);
            }
            let entry = Arc::new(Mutex::new(OwnerCacheEntry {
                value: V::default(),
                owners: HashSet::new(),
            }));
            entries.insert(key.clone(), entry.clone());
            return (entry, false);
        }
    }
}
