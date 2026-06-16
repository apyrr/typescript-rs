use std::collections::BTreeMap;
use std::io;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use crate::vfs::{Entries, FileInfo, Fs};

#[derive(Clone, Debug)]
enum CachedStat {
    Ok(FileInfo),
    Err(io::ErrorKind),
}

impl CachedStat {
    fn from_result(result: &io::Result<FileInfo>) -> Self {
        match result {
            Ok(info) => Self::Ok(info.clone()),
            Err(err) => Self::Err(err.kind()),
        }
    }

    fn into_result(self) -> io::Result<FileInfo> {
        match self {
            Self::Ok(info) => Ok(info),
            Self::Err(kind) => Err(kind.into()),
        }
    }
}

pub struct CachedFs {
    fs: Arc<dyn Fs + Send + Sync>,
    enabled: AtomicBool,
    directory_exists_cache: Mutex<BTreeMap<String, bool>>,
    file_exists_cache: Mutex<BTreeMap<String, bool>>,
    get_accessible_entries_cache: Mutex<BTreeMap<String, Entries>>,
    realpath_cache: Mutex<BTreeMap<String, String>>,
    stat_cache: Mutex<BTreeMap<String, CachedStat>>,
}

impl CachedFs {
    pub fn from(fs: Arc<dyn Fs + Send + Sync>) -> Self {
        Self {
            fs,
            enabled: AtomicBool::new(true),
            directory_exists_cache: Mutex::new(BTreeMap::new()),
            file_exists_cache: Mutex::new(BTreeMap::new()),
            get_accessible_entries_cache: Mutex::new(BTreeMap::new()),
            realpath_cache: Mutex::new(BTreeMap::new()),
            stat_cache: Mutex::new(BTreeMap::new()),
        }
    }

    pub fn disable_and_clear_cache(&self) {
        if self
            .enabled
            .compare_exchange(true, false, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
        {
            self.clear_cache();
        }
    }

    pub fn enable(&self) {
        self.enabled.store(true, Ordering::SeqCst);
    }

    pub fn clear_cache(&self) {
        self.directory_exists_cache.lock().unwrap().clear();
        self.file_exists_cache.lock().unwrap().clear();
        self.get_accessible_entries_cache.lock().unwrap().clear();
        self.realpath_cache.lock().unwrap().clear();
        self.stat_cache.lock().unwrap().clear();
    }

    fn enabled(&self) -> bool {
        self.enabled.load(Ordering::SeqCst)
    }
}

impl Fs for CachedFs {
    fn use_case_sensitive_file_names(&self) -> bool {
        self.fs.use_case_sensitive_file_names()
    }

    fn file_exists(&self, path: &str) -> bool {
        if self.enabled()
            && let Some(value) = self.file_exists_cache.lock().unwrap().get(path).copied()
        {
            return value;
        }
        let value = self.fs.file_exists(path);
        if self.enabled() {
            self.file_exists_cache
                .lock()
                .unwrap()
                .insert(path.to_owned(), value);
        }
        value
    }

    fn read_file(&self, path: &str) -> (String, bool) {
        self.fs.read_file(path)
    }

    fn write_file(&self, path: &str, data: &str) -> io::Result<()> {
        self.fs.write_file(path, data)
    }

    fn append_file(&self, path: &str, data: &str) -> io::Result<()> {
        self.fs.append_file(path, data)
    }

    fn remove(&self, path: &str) -> io::Result<()> {
        self.fs.remove(path)
    }

    fn chtimes(&self, path: &str, atime: SystemTime, mtime: SystemTime) -> io::Result<()> {
        self.fs.chtimes(path, atime, mtime)
    }

    fn directory_exists(&self, path: &str) -> bool {
        if self.enabled()
            && let Some(value) = self
                .directory_exists_cache
                .lock()
                .unwrap()
                .get(path)
                .copied()
        {
            return value;
        }
        let value = self.fs.directory_exists(path);
        if self.enabled() {
            self.directory_exists_cache
                .lock()
                .unwrap()
                .insert(path.to_owned(), value);
        }
        value
    }

    fn get_accessible_entries(&self, path: &str) -> Entries {
        if self.enabled()
            && let Some(value) = self
                .get_accessible_entries_cache
                .lock()
                .unwrap()
                .get(path)
                .cloned()
        {
            return value;
        }
        let value = self.fs.get_accessible_entries(path);
        if self.enabled() {
            self.get_accessible_entries_cache
                .lock()
                .unwrap()
                .insert(path.to_owned(), value.clone());
        }
        value
    }

    fn stat(&self, path: &str) -> io::Result<FileInfo> {
        if self.enabled()
            && let Some(value) = self.stat_cache.lock().unwrap().get(path).cloned()
        {
            return value.into_result();
        }
        let value = self.fs.stat(path);
        if self.enabled() {
            self.stat_cache
                .lock()
                .unwrap()
                .insert(path.to_owned(), CachedStat::from_result(&value));
        }
        value
    }

    fn walk_dir(&self, root: &str, walk_fn: &mut crate::WalkDirFunc<'_>) -> io::Result<()> {
        self.fs.walk_dir(root, walk_fn)
    }

    fn realpath(&self, path: &str) -> String {
        if self.enabled()
            && let Some(value) = self.realpath_cache.lock().unwrap().get(path).cloned()
        {
            return value;
        }
        let value = self.fs.realpath(path);
        if self.enabled() {
            self.realpath_cache
                .lock()
                .unwrap()
                .insert(path.to_owned(), value.clone());
        }
        value
    }
}
