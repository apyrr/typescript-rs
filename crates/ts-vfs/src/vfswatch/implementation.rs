use std::collections::BTreeMap;
use std::io::{self, Write};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

use crate::vfs::{DirEntry, Fs};
use xxhash_rust::xxh3;

pub const DEBOUNCE_WAIT: Duration = Duration::from_millis(250);

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct WatchEntry {
    pub mod_time: Option<SystemTime>,
    pub exists: bool,
    pub children_hash: u64,
}

pub struct FileWatcher {
    fs: Arc<dyn Fs + Send + Sync>,
    testing: bool,
    callback: Box<dyn Fn() + Send + Sync>,
    mu: Mutex<FileWatcherState>,
}

struct FileWatcherState {
    poll_interval: Duration,
    debug_log: Option<Box<dyn Write + Send>>,
    watch_state: Option<BTreeMap<String, WatchEntry>>,
    wildcard_directories: BTreeMap<String, bool>,
}

impl FileWatcher {
    pub fn new(
        fs: Arc<dyn Fs + Send + Sync>,
        poll_interval: Duration,
        testing: bool,
        callback: impl Fn() + Send + Sync + 'static,
    ) -> Self {
        Self {
            fs,
            testing,
            callback: Box::new(callback),
            mu: Mutex::new(FileWatcherState {
                poll_interval,
                debug_log: None,
                watch_state: None,
                wildcard_directories: BTreeMap::new(),
            }),
        }
    }

    pub fn set_debug_log(&self, writer: Option<Box<dyn Write + Send>>) {
        self.mu.lock().unwrap().debug_log = writer;
    }

    pub fn set_poll_interval(&self, duration: Duration) {
        self.mu.lock().unwrap().poll_interval = duration;
    }

    pub fn set_callback(&mut self, callback: impl Fn() + Send + Sync + 'static) {
        self.callback = Box::new(callback);
    }

    pub fn watch_state_entry(&self, path: &str) -> Option<WatchEntry> {
        self.mu
            .lock()
            .unwrap()
            .watch_state
            .as_ref()
            .and_then(|state| state.get(path).cloned())
    }

    pub fn watch_state_uninitialized(&self) -> bool {
        self.mu.lock().unwrap().watch_state.is_none()
    }

    pub fn update_watch_state(&self, paths: &[String], wildcard_dirs: &BTreeMap<String, bool>) {
        let state = snapshot_paths(self.fs.as_ref(), paths, wildcard_dirs);
        let mut guard = self.mu.lock().unwrap();
        guard.watch_state = Some(state);
        guard.wildcard_directories = wildcard_dirs.clone();
    }

    pub fn wait_for_settled(&self, now: impl Fn() -> SystemTime) {
        if self.testing {
            return;
        }
        let poll_interval = self.mu.lock().unwrap().poll_interval;
        let mut current = self.current_state();
        let mut settled_at = now();
        let tick = poll_interval.min(DEBOUNCE_WAIT);
        while now().duration_since(settled_at).unwrap_or_default() < DEBOUNCE_WAIT {
            std::thread::sleep(tick);
            if self.has_changes(&current) {
                current = self.current_state();
                settled_at = now();
            }
        }
    }

    pub fn has_changes_from_watch_state(&self) -> bool {
        let watch_state = self.mu.lock().unwrap().watch_state.clone();
        watch_state
            .as_ref()
            .is_some_and(|baseline| self.has_changes(baseline))
    }

    pub fn run(&self, now: impl Fn() -> SystemTime) {
        loop {
            if self.poll_once(&now) {
                (self.callback)();
            }
        }
    }

    pub fn poll_once(&self, now: impl Fn() -> SystemTime) -> bool {
        let (interval, watch_state) = {
            let guard = self.mu.lock().unwrap();
            (guard.poll_interval, guard.watch_state.clone())
        };
        std::thread::sleep(interval);
        let start = now();
        let changed = watch_state
            .as_ref()
            .map(|baseline| self.has_changes(baseline))
            .unwrap_or(true);
        {
            let mut guard = self.mu.lock().unwrap();
            if let Some(log) = guard.debug_log.as_mut() {
                let elapsed = now().duration_since(start).unwrap_or_default();
                let (mut files, mut dirs, mut missing) = (0, 0, 0);
                if let Some(watch_state) = &watch_state {
                    for entry in watch_state.values() {
                        if !entry.exists {
                            missing += 1;
                        } else if entry.children_hash != 0 {
                            dirs += 1;
                        } else {
                            files += 1;
                        }
                    }
                }
                let total = watch_state.as_ref().map(BTreeMap::len).unwrap_or_default();
                let _ = writeln!(
                    log,
                    "[vfswatch] scan: {total} paths ({files} files, {dirs} dirs, {missing} missing), {:.1}ms, changed={changed}",
                    elapsed.as_micros() as f64 / 1000.0
                );
            }
        }
        if changed {
            self.wait_for_settled(&now);
        }
        changed
    }

    fn current_state(&self) -> BTreeMap<String, WatchEntry> {
        let guard = self.mu.lock().unwrap();
        let paths = guard
            .watch_state
            .as_ref()
            .map(|state| state.keys().cloned().collect::<Vec<_>>())
            .unwrap_or_default();
        let wildcard_directories = guard.wildcard_directories.clone();
        drop(guard);
        snapshot_paths(self.fs.as_ref(), &paths, &wildcard_directories)
    }

    fn has_changes(&self, baseline: &BTreeMap<String, WatchEntry>) -> bool {
        for (path, old) in baseline {
            let stat = self.fs.stat(path).ok();
            if !old.exists {
                if stat.is_some() {
                    return true;
                }
            } else {
                let Some(info) = stat else {
                    return true;
                };
                if info.modified().ok() != old.mod_time {
                    return true;
                }
                if old.children_hash != 0 {
                    let entries = self.fs.get_accessible_entries(path);
                    if hash_entries(&entries.files, &entries.directories) != old.children_hash {
                        return true;
                    }
                }
            }
        }
        false
    }
}

pub fn snapshot_paths(
    fs: &dyn Fs,
    paths: &[String],
    wildcard_dirs: &BTreeMap<String, bool>,
) -> BTreeMap<String, WatchEntry> {
    let mut state = BTreeMap::new();
    for path in paths {
        state.insert(path.clone(), snapshot_path(fs, path));
    }
    for (dir, recursive) in wildcard_dirs {
        if !recursive {
            snapshot_dir_entry(fs, &mut state, dir);
            continue;
        }
        // PORT NOTE: Rust Fs::walk_dir implementations do not visit the root
        // entry today; Go fs.WalkDir does, so snapshot it here to preserve the
        // Go watch state until the shared FS contract is corrected.
        snapshot_dir_entry(fs, &mut state, dir);
        let _ = fs.walk_dir(
            dir,
            &mut |path: &str, entry: DirEntry, err: Option<io::Error>| {
                if err.is_some() || !entry.file_type().is_ok_and(|file_type| file_type.is_dir()) {
                    return Ok(());
                }
                snapshot_dir_entry(fs, &mut state, path);
                Ok(())
            },
        );
    }
    state
}

pub fn snapshot_dir_entry(fs: &dyn Fs, state: &mut BTreeMap<String, WatchEntry>, dir: &str) {
    let entries = fs.get_accessible_entries(dir);
    let hash = hash_entries(&entries.files, &entries.directories);
    if let Some(existing) = state.get_mut(dir) {
        existing.children_hash = hash;
    } else if let Ok(info) = fs.stat(dir) {
        state.insert(
            dir.to_owned(),
            WatchEntry {
                mod_time: info.modified().ok(),
                exists: true,
                children_hash: hash,
            },
        );
    }
}

pub fn hash_entries(files: &[String], directories: &[String]) -> u64 {
    let mut directories = directories.to_vec();
    let mut files = files.to_vec();
    directories.sort();
    files.sort();
    let mut hash = xxh3::Xxh3::new();
    for (prefix, values) in [("d:", directories), ("f:", files)] {
        for value in values {
            hash.update(prefix.as_bytes());
            hash.update(value.as_bytes());
            hash.update(&[0]);
        }
    }
    hash.digest()
}

fn snapshot_path(fs: &dyn Fs, path: &str) -> WatchEntry {
    if let Ok(info) = fs.stat(path) {
        WatchEntry {
            mod_time: info.modified().ok(),
            exists: true,
            children_hash: 0,
        }
    } else {
        WatchEntry {
            mod_time: None,
            exists: false,
            children_hash: 0,
        }
    }
}
