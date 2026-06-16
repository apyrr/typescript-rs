#[cfg(feature = "link_store_stats")]
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::{
    cell::RefCell,
    collections::hash_map::Entry,
    fmt,
    hash::{Hash, Hasher},
};

use ts_collections::{Arena, FastHashMap as HashMap, FastHashMapExt, Idx};

// Links store

pub struct LinkStore<K, V> {
    inner: RefCell<LinkStoreInner<K, V>>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct LinkStoreStatsSnapshot {
    pub ensure_handle: u64,
    pub ensure_handle_hit: u64,
    pub ensure_handle_miss: u64,
    pub with_by_handle: u64,
    pub with_by_handle_mut: u64,
}

#[cfg(feature = "link_store_stats")]
static LINK_STORE_STATS_ENABLED: AtomicBool = AtomicBool::new(false);
#[cfg(feature = "link_store_stats")]
static LINK_STORE_ENSURE_HANDLE: AtomicU64 = AtomicU64::new(0);
#[cfg(feature = "link_store_stats")]
static LINK_STORE_ENSURE_HANDLE_HIT: AtomicU64 = AtomicU64::new(0);
#[cfg(feature = "link_store_stats")]
static LINK_STORE_ENSURE_HANDLE_MISS: AtomicU64 = AtomicU64::new(0);
#[cfg(feature = "link_store_stats")]
static LINK_STORE_WITH_BY_HANDLE: AtomicU64 = AtomicU64::new(0);
#[cfg(feature = "link_store_stats")]
static LINK_STORE_WITH_BY_HANDLE_MUT: AtomicU64 = AtomicU64::new(0);

pub const fn link_store_stats_available() -> bool {
    cfg!(feature = "link_store_stats")
}

pub fn set_link_store_stats_enabled(enabled: bool) {
    #[cfg(feature = "link_store_stats")]
    LINK_STORE_STATS_ENABLED.store(enabled, Ordering::Relaxed);

    #[cfg(not(feature = "link_store_stats"))]
    let _ = enabled;
}

pub fn reset_link_store_stats() {
    #[cfg(feature = "link_store_stats")]
    {
        LINK_STORE_ENSURE_HANDLE.store(0, Ordering::Relaxed);
        LINK_STORE_ENSURE_HANDLE_HIT.store(0, Ordering::Relaxed);
        LINK_STORE_ENSURE_HANDLE_MISS.store(0, Ordering::Relaxed);
        LINK_STORE_WITH_BY_HANDLE.store(0, Ordering::Relaxed);
        LINK_STORE_WITH_BY_HANDLE_MUT.store(0, Ordering::Relaxed);
    }
}

pub fn link_store_stats_snapshot() -> LinkStoreStatsSnapshot {
    #[cfg(feature = "link_store_stats")]
    {
        return LinkStoreStatsSnapshot {
            ensure_handle: LINK_STORE_ENSURE_HANDLE.load(Ordering::Relaxed),
            ensure_handle_hit: LINK_STORE_ENSURE_HANDLE_HIT.load(Ordering::Relaxed),
            ensure_handle_miss: LINK_STORE_ENSURE_HANDLE_MISS.load(Ordering::Relaxed),
            with_by_handle: LINK_STORE_WITH_BY_HANDLE.load(Ordering::Relaxed),
            with_by_handle_mut: LINK_STORE_WITH_BY_HANDLE_MUT.load(Ordering::Relaxed),
        };
    }

    #[cfg(not(feature = "link_store_stats"))]
    LinkStoreStatsSnapshot::default()
}

#[cfg(feature = "link_store_stats")]
#[inline(always)]
fn record_link_store_stat(counter: &AtomicU64) {
    if LINK_STORE_STATS_ENABLED.load(Ordering::Relaxed) {
        counter.fetch_add(1, Ordering::Relaxed);
    }
}

pub struct LinkHandle<V>(Idx<V>);

impl<V> Clone for LinkHandle<V> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<V> Copy for LinkHandle<V> {}

impl<V> PartialEq for LinkHandle<V> {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl<V> Eq for LinkHandle<V> {}

impl<V> PartialOrd for LinkHandle<V> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl<V> Ord for LinkHandle<V> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.cmp(&other.0)
    }
}

impl<V> Hash for LinkHandle<V> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

impl<V> fmt::Debug for LinkHandle<V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("LinkHandle")
            .field(&self.0.into_raw())
            .finish()
    }
}

struct LinkStoreInner<K, V> {
    entries: HashMap<K, Idx<V>>,
    arena: Arena<V>,
}

pub trait IntoLinkKey<K> {
    fn into_link_key(self) -> K;
}

impl<K> IntoLinkKey<K> for K {
    fn into_link_key(self) -> K {
        self
    }
}

impl<K: Copy> IntoLinkKey<K> for &K {
    fn into_link_key(self) -> K {
        *self
    }
}

impl<K, V> Default for LinkStore<K, V> {
    fn default() -> Self {
        Self {
            inner: RefCell::new(LinkStoreInner {
                entries: HashMap::new(),
                arena: Arena::new(),
            }),
        }
    }
}

impl<K, V> LinkStore<K, V>
where
    K: Eq + Hash,
    V: Default,
{
    pub fn is_empty(&self) -> bool {
        self.inner.borrow().entries.is_empty()
    }

    pub fn ensure_handle<Q>(&self, key: Q) -> LinkHandle<V>
    where
        Q: IntoLinkKey<K>,
    {
        #[cfg(feature = "link_store_stats")]
        record_link_store_stat(&LINK_STORE_ENSURE_HANDLE);

        let key = key.into_link_key();
        let mut inner = self.inner.borrow_mut();
        let LinkStoreInner { entries, arena } = &mut *inner;
        let idx = match entries.entry(key) {
            Entry::Occupied(entry) => {
                #[cfg(feature = "link_store_stats")]
                record_link_store_stat(&LINK_STORE_ENSURE_HANDLE_HIT);

                *entry.get()
            }
            Entry::Vacant(entry) => {
                #[cfg(feature = "link_store_stats")]
                record_link_store_stat(&LINK_STORE_ENSURE_HANDLE_MISS);

                let idx = arena.alloc(V::default());
                entry.insert(idx);
                idx
            }
        };
        LinkHandle(idx)
    }

    pub fn allocate_unkeyed_handle(&self) -> LinkHandle<V> {
        let mut inner = self.inner.borrow_mut();
        LinkHandle(inner.arena.alloc(V::default()))
    }

    pub fn has<Q>(&self, key: Q) -> bool
    where
        Q: IntoLinkKey<K>,
    {
        let key = key.into_link_key();
        self.inner.borrow().entries.contains_key(&key)
    }

    pub fn try_handle<Q>(&self, key: Q) -> Option<LinkHandle<V>>
    where
        Q: IntoLinkKey<K>,
    {
        let key = key.into_link_key();
        self.inner
            .borrow()
            .entries
            .get(&key)
            .copied()
            .map(LinkHandle)
    }

    pub fn with_by_handle<R>(&self, handle: LinkHandle<V>, f: impl FnOnce(&V) -> R) -> R {
        #[cfg(feature = "link_store_stats")]
        record_link_store_stat(&LINK_STORE_WITH_BY_HANDLE);

        let inner = self.inner.borrow();
        let links = inner
            .arena
            .get(handle.0)
            .expect("link handle must resolve in this link store");
        f(links)
    }

    pub fn with_by_handle_mut<R>(&self, handle: LinkHandle<V>, f: impl FnOnce(&mut V) -> R) -> R {
        #[cfg(feature = "link_store_stats")]
        record_link_store_stat(&LINK_STORE_WITH_BY_HANDLE_MUT);

        let mut inner = self.inner.borrow_mut();
        let links = inner
            .arena
            .get_mut(handle.0)
            .expect("link handle must resolve in this link store");
        f(links)
    }

    pub fn extend_from(&mut self, other: LinkStore<K, V>) {
        let LinkStoreInner {
            entries,
            arena: other_arena,
        } = other.inner.into_inner();
        let mut values: Vec<Option<V>> = other_arena
            .into_iter()
            .map(|(_idx, value)| Some(value))
            .collect();
        let inner = self.inner.get_mut();

        for (key, old_idx) in entries {
            let old_index = old_idx.into_raw().into_usize();
            let value = values
                .get_mut(old_index)
                .and_then(Option::take)
                .expect("link store entry index must resolve while extending");
            let new_idx = inner.arena.alloc(value);
            inner.entries.insert(key, new_idx);
        }
    }
}
