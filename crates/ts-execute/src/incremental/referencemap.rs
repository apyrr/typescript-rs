use std::{
    collections::HashMap,
    sync::{Mutex, OnceLock},
};

use ts_collections::{Set, SyncMap};
use ts_tspath as tspath;

struct ReferenceMap {
    references: SyncMap<tspath::Path, Set<tspath::Path>>,
    referenced_by: OnceLock<HashMap<tspath::Path, Set<tspath::Path>>>,
    lock: Mutex<()>,
}

impl Default for ReferenceMap {
    fn default() -> Self {
        Self {
            references: SyncMap::default(),
            referenced_by: OnceLock::new(),
            lock: Mutex::new(()),
        }
    }
}

impl Clone for ReferenceMap {
    fn clone(&self) -> Self {
        let cloned = Self::default();
        self.references.range(|key, value| {
            cloned.references.store(key.clone(), value.cloned());
            true
        });
        cloned
    }
}

impl ReferenceMap {
    fn store_references(&self, path: tspath::Path, refs: Set<tspath::Path>) {
        self.references.store(path, Some(refs));
    }

    fn get_references(&self, path: &tspath::Path) -> (Option<Set<tspath::Path>>, bool) {
        self.references.load(path)
    }

    fn get_paths_with_references(&self) -> Vec<tspath::Path> {
        self.references.keys()
    }

    fn get_referenced_by(&self, path: &tspath::Path) -> Vec<tspath::Path> {
        let referenced_by = self.referenced_by.get_or_init(|| {
            let _guard = self.lock.lock().unwrap_or_else(|err| err.into_inner());
            let mut referenced_by = HashMap::<tspath::Path, Set<tspath::Path>>::new();
            self.references.range(|key, value| {
                if let Some(value) = value {
                    for ref_ in value.keys().into_iter().flatten() {
                        referenced_by
                            .entry(ref_.clone())
                            .or_default()
                            .add(key.clone());
                    }
                }
                true
            });
            referenced_by
        });
        referenced_by
            .get(path)
            .map(|refs| refs.keys().into_iter().flatten().cloned().collect())
            .unwrap_or_default()
    }
}
