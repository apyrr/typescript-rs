use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use ts_tsoptions::tsconfigparsing;
use ts_tspath as tspath;

pub use ts_tsoptions::tsconfigparsing::{ExtendedConfigCacheEntry, ParseConfigHost};

// extendedConfigCache is a minimal implementation of tsoptions.ExtendedConfigCache.
// It is concurrency-safe, but stores cached entries permanently. This implementation
// should not be used for long-running processes where configuration changes over the
// course of multiple compilations.
#[derive(Default)]
pub struct ExtendedConfigCache {
    m: Mutex<HashMap<tspath::Path, Arc<ExtendedConfigCacheEntrySlot>>>,
}

#[derive(Default)]
struct ExtendedConfigCacheEntrySlot {
    mu: Mutex<ExtendedConfigCacheEntryData>,
}

#[derive(Default)]
struct ExtendedConfigCacheEntryData {
    extended_config_cache_entry: Option<ExtendedConfigCacheEntry>,
}

impl ExtendedConfigCache {
    // GetExtendedConfig implements tsoptions.ExtendedConfigCache.
    pub fn get_extended_config(
        &self,
        file_name: String,
        path: tspath::Path,
        resolution_stack: Vec<String>,
        host: &dyn ParseConfigHost,
    ) -> ExtendedConfigCacheEntry {
        self.load_or_store_new_locked_entry(path.clone(), |entry, loaded| {
            if !loaded {
                entry.extended_config_cache_entry = Some(tsconfigparsing::parse_extended_config(
                    &file_name,
                    path,
                    &resolution_stack,
                    host,
                    Some(self),
                ));
            }
            entry.extended_config_cache_entry.clone().unwrap()
        })
    }

    // loadOrStoreNewLockedEntry loads an existing entry or creates a new one. The returned entry's mutex is locked.
    fn load_or_store_new_locked_entry<R>(
        &self,
        path: tspath::Path,
        f: impl FnOnce(&mut ExtendedConfigCacheEntryData, bool) -> R,
    ) -> R {
        // PORT NOTE: reshaped for borrowck; Go returns the locked entry, while
        // this keeps the mutex guard inside the helper and runs `f` under it.
        let mut map = self.m.lock().unwrap_or_else(|err| err.into_inner());
        if let Some(existing) = map.get(&path) {
            let entry = Arc::clone(existing);
            drop(map);
            let mut entry = entry.mu.lock().unwrap_or_else(|err| err.into_inner());
            return f(&mut entry, true);
        }

        let entry = Arc::new(ExtendedConfigCacheEntrySlot::default());
        let mut locked_entry = entry.mu.lock().unwrap_or_else(|err| err.into_inner());
        map.insert(path, Arc::clone(&entry));
        drop(map);
        f(&mut locked_entry, false)
    }
}

impl tsconfigparsing::ExtendedConfigCache for ExtendedConfigCache {
    fn get_extended_config(
        &self,
        file_name: String,
        path: tspath::Path,
        resolution_stack: Vec<String>,
        host: &dyn tsconfigparsing::ParseConfigHost,
    ) -> tsconfigparsing::ExtendedConfigCacheEntry {
        ExtendedConfigCache::get_extended_config(self, file_name, path, resolution_stack, host)
    }
}
