use std::{
    collections::HashMap,
    hash::Hash,
    sync::{Arc, Mutex},
};

use ts_ast as ast;
use ts_tsoptions as tsoptions;

struct ParseCacheEntry<V> {
    value: Mutex<Option<V>>,
}

pub(crate) struct ParseCache<K, V> {
    entries: Mutex<HashMap<K, Arc<ParseCacheEntry<V>>>>,
}

pub(crate) trait IsZero {
    fn is_zero(&self) -> bool;
}

impl<T> IsZero for Option<T> {
    fn is_zero(&self) -> bool {
        self.is_none()
    }
}

pub(crate) trait ParseCacheValue: Default + IsZero {
    fn share_for_parse_cache(&self) -> Self;
}

impl ParseCacheValue for Option<ast::ParsedSourceFile> {
    fn share_for_parse_cache(&self) -> Self {
        self.as_ref().map(ast::ParsedSourceFile::share_readonly)
    }
}

impl ParseCacheValue for Option<tsoptions::ParsedCommandLine> {
    fn share_for_parse_cache(&self) -> Self {
        self.clone()
    }
}

impl<K, V> Default for ParseCache<K, V> {
    fn default() -> Self {
        Self {
            entries: Mutex::new(HashMap::new()),
        }
    }
}

impl<K, V> ParseCache<K, V>
where
    K: Eq + Hash + Clone,
    V: ParseCacheValue,
{
    pub(crate) fn load_or_store(&self, key: K, parse: impl FnOnce(K) -> V, allow_zero: bool) -> V {
        let new_entry = Arc::new(ParseCacheEntry {
            value: Mutex::new(None),
        });
        let (entry, loaded) = {
            let mut entries = self.entries.lock().unwrap_or_else(|err| err.into_inner());
            if let Some(entry) = entries.get(&key) {
                (Arc::clone(entry), true)
            } else {
                entries.insert(key.clone(), Arc::clone(&new_entry));
                (new_entry, false)
            }
        };

        let mut value = entry.value.lock().unwrap_or_else(|err| err.into_inner());
        if loaded {
            if let Some(existing) = value.as_ref() {
                if allow_zero || !existing.is_zero() {
                    return existing.share_for_parse_cache();
                }
            }
        }
        let parsed = parse(key);
        *value = Some(parsed.share_for_parse_cache());
        parsed
    }
}

impl<K, V> ParseCache<K, V>
where
    K: Eq + Hash,
{
    pub(crate) fn store(&self, key: K, value: V) {
        let mut entries = self.entries.lock().unwrap_or_else(|err| err.into_inner());
        entries.insert(
            key,
            Arc::new(ParseCacheEntry {
                value: Mutex::new(Some(value)),
            }),
        );
    }

    pub(crate) fn delete(&self, key: &K) {
        self.entries
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .remove(key);
    }

    pub(crate) fn reset(&self) {
        self.entries
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .clear();
    }
}
