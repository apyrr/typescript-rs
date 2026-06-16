use std::{
    collections::{HashMap, HashSet},
    io,
    sync::{
        Arc, Mutex, OnceLock, RwLock,
        atomic::{AtomicBool, Ordering},
    },
    time::SystemTime,
};

use ts_collections as collections;
use ts_ls as lsconv;
use ts_lsproto::{self as lsproto, DocumentUriExt};
use ts_tspath as tspath;
use ts_vfs::{self as vfs, cachedvfs};

use crate::FileChangeSummary;
use crate::dirty;
use crate::overlayfs::{DiskFile, FileContent, FileHandle, Overlay, new_disk_file};

pub type FileHandleRef = Arc<dyn FileHandle + Send + Sync>;
pub(crate) type ToPath = Arc<dyn Fn(&str) -> tspath::Path + Send + Sync>;
pub(crate) type MemoizedDiskFile = OnceLock<Option<FileHandleRef>>;

pub trait FileSource: Send {
    fn fs(&self) -> Arc<dyn vfs::Fs + Send + Sync>;
    fn get_file(&mut self, file_name: &str) -> Option<FileHandleRef>;
    fn get_file_by_path(&mut self, file_name: &str, path: &tspath::Path) -> Option<FileHandleRef>;
    fn file_exists(&mut self, file_name: &str, path: &tspath::Path) -> bool;
    fn get_accessible_entries(&mut self, path: &str) -> vfs::Entries;
}

// realpathAliasSet is a thread-safe set of symlink paths that alias a single realpath.
// It implements dirty.Cloneable so it can be used as a value in dirty.SyncMap.
#[derive(Default)]
pub struct RealpathAliasSet {
    mu: Mutex<()>,
    pub paths: collections::Set<tspath::Path>,
}

impl RealpathAliasSet {
    pub fn add(&mut self, path: tspath::Path) {
        let _lock = self.mu.lock().unwrap_or_else(|err| err.into_inner());
        self.paths.add(path);
    }

    pub fn clone_set(&self) -> RealpathAliasSet {
        let _lock = self.mu.lock().unwrap_or_else(|err| err.into_inner());
        RealpathAliasSet {
            mu: Mutex::new(()),
            paths: self.paths.clone(),
        }
    }
}

impl Clone for RealpathAliasSet {
    fn clone(&self) -> Self {
        self.clone_set()
    }
}

pub struct SnapshotFs {
    pub(crate) to_path: ToPath,
    pub(crate) fs: Arc<dyn vfs::Fs + Send + Sync>,
    pub(crate) overlays: HashMap<tspath::Path, Arc<Overlay>>,
    pub(crate) overlay_directories: HashMap<tspath::Path, HashMap<tspath::Path, String>>,
    pub(crate) disk_files: HashMap<tspath::Path, Arc<DiskFile>>,
    pub(crate) disk_directories: HashMap<tspath::Path, dirty::CloneableMap<tspath::Path, String>>,
    pub(crate) read_files: Mutex<HashMap<tspath::Path, Arc<MemoizedDiskFile>>>,
    // nodeModulesRealpathAliases maps realpath-based keys to sets of symlink-based keys,
    // for files inside node_modules that are accessed through directory symlinks.
    // This allows watch events (which use realpaths) to invalidate files cached under symlink paths.
    pub(crate) node_modules_realpath_aliases: HashMap<tspath::Path, RealpathAliasSet>,
}

impl Clone for SnapshotFs {
    fn clone(&self) -> Self {
        Self {
            to_path: self.to_path.clone(),
            fs: self.fs.clone(),
            overlays: self.overlays.clone(),
            overlay_directories: self.overlay_directories.clone(),
            disk_files: self.disk_files.clone(),
            disk_directories: self.disk_directories.clone(),
            read_files: Mutex::new(
                self.read_files
                    .lock()
                    .unwrap_or_else(|err| err.into_inner())
                    .clone(),
            ),
            node_modules_realpath_aliases: self.node_modules_realpath_aliases.clone(),
        }
    }
}

impl FileSource for SnapshotFs {
    fn fs(&self) -> Arc<dyn vfs::Fs + Send + Sync> {
        self.fs.clone()
    }

    fn get_file(&mut self, file_name: &str) -> Option<FileHandleRef> {
        self.get_file_by_path(file_name, &(self.to_path)(file_name))
    }

    fn get_file_by_path(&mut self, file_name: &str, path: &tspath::Path) -> Option<FileHandleRef> {
        if let Some(file) = self.overlays.get(path) {
            return Some(file.clone());
        }
        if let Some(file) = self.disk_files.get(path) {
            return Some(file.clone());
        }
        let entry = {
            let mut read_files = self
                .read_files
                .lock()
                .unwrap_or_else(|err| err.into_inner());
            read_files
                .entry(path.clone())
                .or_insert_with(|| Arc::new(OnceLock::new()))
                .clone()
        };
        entry
            .get_or_init(|| {
                let (contents, ok) = self.fs.read_file(file_name);
                if ok {
                    Some(Arc::new(new_disk_file(file_name.to_owned(), contents)))
                } else {
                    None
                }
            })
            .clone()
    }

    fn file_exists(&mut self, file_name: &str, path: &tspath::Path) -> bool {
        self.overlays.contains_key(path)
            || self.disk_files.contains_key(path)
            || self.fs.file_exists(file_name)
    }

    fn get_accessible_entries(&mut self, directory_name: &str) -> vfs::Entries {
        let mut entries = vfs::Entries::default();
        let path = (self.to_path)(directory_name);
        if let Some(disk_directories) = self.disk_directories.get(&path) {
            read_directory_into_entries(disk_directories, |path| self.is_file(path), &mut entries);
        }
        if let Some(overlay_directories) = self.overlay_directories.get(&path) {
            read_directory_into_entries(
                overlay_directories,
                |path| self.is_file(path),
                &mut entries,
            );
        }
        entries
    }
}

impl SnapshotFs {
    pub fn fs(&self) -> Arc<dyn vfs::Fs + Send + Sync> {
        <Self as FileSource>::fs(self)
    }

    pub fn get_file(&self, file_name: &str) -> Option<FileHandleRef> {
        self.get_file_by_path(file_name, &(self.to_path)(file_name))
    }

    pub fn get_file_by_path(&self, file_name: &str, path: &tspath::Path) -> Option<FileHandleRef> {
        if let Some(file) = self.overlays.get(path) {
            return Some(file.clone());
        }
        if let Some(file) = self.disk_files.get(path) {
            return Some(file.clone());
        }
        let entry = {
            let mut read_files = self
                .read_files
                .lock()
                .unwrap_or_else(|err| err.into_inner());
            read_files
                .entry(path.clone())
                .or_insert_with(|| Arc::new(OnceLock::new()))
                .clone()
        };
        entry
            .get_or_init(|| {
                let (contents, ok) = self.fs.read_file(file_name);
                ok.then(|| Arc::new(new_disk_file(file_name.to_owned(), contents)) as FileHandleRef)
            })
            .clone()
    }

    pub fn file_exists(&self, file_name: &str, path: &tspath::Path) -> bool {
        self.overlays.contains_key(path)
            || self.disk_files.contains_key(path)
            || self.fs.file_exists(file_name)
    }

    pub fn get_accessible_entries(&self, directory_name: &str) -> vfs::Entries {
        let mut entries = vfs::Entries::default();
        let path = (self.to_path)(directory_name);
        if let Some(disk_directories) = self.disk_directories.get(&path) {
            read_directory_into_entries(disk_directories, |path| self.is_file(path), &mut entries);
        }
        if let Some(overlay_directories) = self.overlay_directories.get(&path) {
            read_directory_into_entries(
                overlay_directories,
                |path| self.is_file(path),
                &mut entries,
            );
        }
        entries
    }

    pub fn is_open_file(&self, file_name: &str) -> bool {
        let path = (self.to_path)(file_name);
        self.overlays.contains_key(&path)
    }

    fn is_file(&self, path: &tspath::Path) -> bool {
        self.disk_files.contains_key(path) || self.overlays.contains_key(path)
    }

    pub fn expand_realpath_aliases(&self, mut change: FileChangeSummary) -> FileChangeSummary {
        if self.node_modules_realpath_aliases.is_empty() {
            return change;
        }

        let mut additional_changed = HashSet::new();
        for uri in &change.changed {
            let path = (self.to_path)(&uri.file_name());
            if let Some(aliases) = self.node_modules_realpath_aliases.get(&path) {
                for alias_path in aliases.paths.keys().into_iter().flatten() {
                    additional_changed.insert(lsconv::file_name_to_document_uri(alias_path));
                }
            }
        }
        change.changed.extend(additional_changed);

        let mut additional_deleted = HashSet::new();
        for uri in &change.deleted {
            let path = (self.to_path)(&uri.file_name());
            if let Some(aliases) = self.node_modules_realpath_aliases.get(&path) {
                for alias_path in aliases.paths.keys().into_iter().flatten() {
                    additional_deleted.insert(lsconv::file_name_to_document_uri(alias_path));
                }
            }
        }
        change.deleted.extend(additional_deleted);

        change
    }
}

pub struct SnapshotFsBuilder {
    pub(crate) fs: Arc<dyn vfs::Fs + Send + Sync>,
    pub(crate) prev_overlays: HashMap<tspath::Path, Arc<Overlay>>,
    pub(crate) overlays: HashMap<tspath::Path, Arc<Overlay>>,
    pub(crate) overlay_directories: HashMap<tspath::Path, HashMap<tspath::Path, String>>,
    pub(crate) disk_files: BuilderDiskFiles,
    pub(crate) disk_directories: HashMap<tspath::Path, dirty::CloneableMap<tspath::Path, String>>,
    pub(crate) node_modules_realpath_aliases: BuilderRealpathAliases,
    pub(crate) to_path: ToPath,
}

impl Clone for SnapshotFsBuilder {
    fn clone(&self) -> Self {
        Self {
            fs: self.fs.clone(),
            prev_overlays: self.prev_overlays.clone(),
            overlays: self.overlays.clone(),
            overlay_directories: self.overlay_directories.clone(),
            disk_files: self.disk_files.clone(),
            disk_directories: self.disk_directories.clone(),
            node_modules_realpath_aliases: self.node_modules_realpath_aliases.clone(),
            to_path: self.to_path.clone(),
        }
    }
}

pub(crate) struct BuilderDiskFiles {
    state: Arc<Mutex<BuilderDiskFilesState>>,
}

struct BuilderDiskFilesState {
    base: HashMap<tspath::Path, Arc<DiskFile>>,
    dirty: HashMap<tspath::Path, Option<Arc<DiskFile>>>,
}

impl Clone for BuilderDiskFiles {
    fn clone(&self) -> Self {
        Self {
            state: self.state.clone(),
        }
    }
}

impl BuilderDiskFiles {
    fn new(base: HashMap<tspath::Path, Arc<DiskFile>>) -> Self {
        Self {
            state: Arc::new(Mutex::new(BuilderDiskFilesState {
                base,
                dirty: HashMap::new(),
            })),
        }
    }

    pub(crate) fn load(&self, key: &tspath::Path) -> Option<BuilderDiskFileEntry> {
        let state = self.state.lock().unwrap_or_else(|err| err.into_inner());
        if let Some(value) = state.dirty.get(key) {
            return value.as_ref().map(|_| BuilderDiskFileEntry {
                key: key.clone(),
                state: self.state.clone(),
            });
        }
        state.base.contains_key(key).then(|| BuilderDiskFileEntry {
            key: key.clone(),
            state: self.state.clone(),
        })
    }

    pub(crate) fn range(&self, mut f: impl FnMut(&BuilderDiskFileEntry) -> bool) {
        for key in self.keys() {
            if let Some(entry) = self.load(&key) {
                if !f(&entry) {
                    break;
                }
            }
        }
    }

    pub(crate) fn value(&self, key: &tspath::Path) -> Option<Arc<DiskFile>> {
        let state = self.state.lock().unwrap_or_else(|err| err.into_inner());
        match state.dirty.get(key) {
            Some(value) => value.clone(),
            None => state.base.get(key).cloned(),
        }
    }

    fn original(&self, key: &tspath::Path) -> Option<Arc<DiskFile>> {
        self.state
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .base
            .get(key)
            .cloned()
    }

    fn store(&mut self, key: tspath::Path, value: Arc<DiskFile>) {
        self.state
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .dirty
            .insert(key, Some(value));
    }

    fn delete(&mut self, key: &tspath::Path) {
        self.state
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .dirty
            .insert(key.clone(), None);
    }

    fn has_changes(&self) -> bool {
        !self
            .state
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .dirty
            .is_empty()
    }

    fn keys(&self) -> Vec<tspath::Path> {
        let state = self.state.lock().unwrap_or_else(|err| err.into_inner());
        let mut keys: HashSet<_> = state.base.keys().cloned().collect();
        for (key, value) in &state.dirty {
            if value.is_some() {
                keys.insert(key.clone());
            } else {
                keys.remove(key);
            }
        }
        keys.into_iter().collect()
    }

    fn dirty_entries(&self) -> HashMap<tspath::Path, Option<Arc<DiskFile>>> {
        self.state
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .dirty
            .clone()
    }

    fn base_contains_key(&self, key: &tspath::Path) -> bool {
        self.state
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .base
            .contains_key(key)
    }

    fn finalize(self) -> HashMap<tspath::Path, Arc<DiskFile>> {
        let state = self.state.lock().unwrap_or_else(|err| err.into_inner());
        let mut result = state.base.clone();
        for (key, value) in &state.dirty {
            if let Some(value) = value {
                result.insert(key.clone(), value.clone());
            } else {
                result.remove(key);
            }
        }
        result
    }
}

#[derive(Clone)]
pub(crate) struct BuilderDiskFileEntry {
    key: tspath::Path,
    state: Arc<Mutex<BuilderDiskFilesState>>,
}

impl BuilderDiskFileEntry {
    pub(crate) fn delete(&self) {
        self.state
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .dirty
            .insert(self.key.clone(), None);
    }

    pub(crate) fn key(&self) -> &tspath::Path {
        &self.key
    }
}

#[derive(Clone)]
pub(crate) struct BuilderRealpathAliases {
    state: Arc<Mutex<HashMap<tspath::Path, RealpathAliasSet>>>,
}

impl BuilderRealpathAliases {
    fn new(base: HashMap<tspath::Path, RealpathAliasSet>) -> Self {
        Self {
            state: Arc::new(Mutex::new(base)),
        }
    }

    fn contains_key(&self, key: &tspath::Path) -> bool {
        self.state
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .contains_key(key)
    }

    fn add(&self, realpath_path: tspath::Path, symlink_path: tspath::Path) {
        self.state
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .entry(realpath_path)
            .or_default()
            .add(symlink_path);
    }

    fn remove_alias_path(&self, realpath_path: &tspath::Path, alias_path: &tspath::Path) {
        let mut state = self.state.lock().unwrap_or_else(|err| err.into_inner());
        if let Some(aliases) = state.get_mut(realpath_path) {
            aliases.paths.delete(alias_path);
            if aliases.paths.len() == 0 {
                state.remove(realpath_path);
            }
        }
    }

    fn finalize(self) -> HashMap<tspath::Path, RealpathAliasSet> {
        self.state
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .clone()
    }
}

pub fn new_snapshot_fs_builder(
    fs: Arc<dyn vfs::Fs + Send + Sync>,
    prev_overlays: HashMap<tspath::Path, Arc<Overlay>>,
    overlays: HashMap<tspath::Path, Arc<Overlay>>,
    disk_files: HashMap<tspath::Path, Arc<DiskFile>>,
    disk_directories: HashMap<tspath::Path, dirty::CloneableMap<tspath::Path, String>>,
    node_modules_realpath_aliases: HashMap<tspath::Path, RealpathAliasSet>,
    _position_encoding: lsproto::PositionEncodingKind,
    to_path: ToPath,
) -> SnapshotFsBuilder {
    let cached_fs = Arc::new(cachedvfs::CachedFs::from(fs));
    cached_fs.enable();

    let mut overlay_directories = HashMap::new();
    for (path, overlay) in &overlays {
        let mut child_path = path.clone();
        let mut child = overlay.file_name();
        loop {
            let parent_path = tspath::get_directory_path(&child_path);
            let parent = tspath::get_directory_path(&child);
            if child_path == parent_path {
                break;
            }
            let base_name = tspath::get_base_file_name(&child);
            overlay_directories
                .entry(parent_path.clone())
                .or_insert_with(HashMap::new)
                .insert(child_path.clone(), base_name);
            child_path = parent_path;
            child = parent;
        }
    }

    SnapshotFsBuilder {
        fs: cached_fs,
        prev_overlays,
        overlays,
        overlay_directories,
        disk_files: BuilderDiskFiles::new(disk_files),
        disk_directories,
        node_modules_realpath_aliases: BuilderRealpathAliases::new(node_modules_realpath_aliases),
        to_path,
    }
}

impl FileSource for SnapshotFsBuilder {
    fn fs(&self) -> Arc<dyn vfs::Fs + Send + Sync> {
        self.fs.clone()
    }

    fn get_file(&mut self, file_name: &str) -> Option<FileHandleRef> {
        let path = (self.to_path)(file_name);
        self.get_file_by_path(file_name, &path)
    }

    fn get_file_by_path(&mut self, file_name: &str, path: &tspath::Path) -> Option<FileHandleRef> {
        if let Some(file) = self.overlays.get(path) {
            return Some(file.clone());
        }
        self.get_disk_file(file_name, path, false)
    }

    fn file_exists(&mut self, file_name: &str, path: &tspath::Path) -> bool {
        if self.overlays.contains_key(path) {
            return true;
        }
        if self.disk_files.load(path).is_some() {
            return self.reload_entry_if_needed_path(path).is_some();
        }
        self.fs.file_exists(file_name)
    }

    fn get_accessible_entries(&mut self, path: &str) -> vfs::Entries {
        let mut entries = self.fs.get_accessible_entries(path);
        if let Some(overlay_directories) = self.overlay_directories.get(&(self.to_path)(path)) {
            read_directory_into_entries(
                overlay_directories,
                |path| self.is_open_file(path),
                &mut entries,
            );
        }
        entries
    }
}

impl SnapshotFsBuilder {
    pub fn fs(&self) -> Arc<dyn vfs::Fs + Send + Sync> {
        <Self as FileSource>::fs(self)
    }

    pub fn finalize(mut self) -> (SnapshotFs, bool) {
        let changed = self.disk_files.has_changes();
        let dirty_entries = self.disk_files.dirty_entries();
        let deleted: Vec<_> = dirty_entries
            .iter()
            .filter_map(|(path, value)| value.is_none().then(|| path.clone()))
            .collect();
        let added: Vec<_> = dirty_entries
            .iter()
            .filter_map(|(path, value)| {
                value
                    .as_ref()
                    .filter(|_| !self.disk_files.base_contains_key(path))
                    .map(|file| (path.clone(), file.clone()))
            })
            .collect();

        for (path, file) in added {
            self.on_added_file(path, file.file_name());
        }

        for path in &deleted {
            self.on_deleted_file_or_directory(path.clone());
        }

        for deleted_path in &deleted {
            if let Some(deleted_file) = self.disk_files.original(deleted_path) {
                if !deleted_file.realpath_path.is_empty() {
                    self.node_modules_realpath_aliases
                        .remove_alias_path(&deleted_file.realpath_path, deleted_path);
                }
            }
        }
        let disk_files = self.disk_files.finalize();
        let node_modules_realpath_aliases = self.node_modules_realpath_aliases.finalize();

        (
            SnapshotFs {
                fs: self.fs,
                overlays: self.overlays,
                overlay_directories: self.overlay_directories,
                disk_files,
                disk_directories: self.disk_directories,
                node_modules_realpath_aliases,
                to_path: self.to_path,
                read_files: Mutex::new(HashMap::new()),
            },
            changed,
        )
    }

    fn on_added_file(&mut self, path: tspath::Path, file_name: String) {
        let mut child_path = path;
        let mut child = file_name;
        loop {
            let parent_path = tspath::get_directory_path(&child_path);
            let parent = tspath::get_directory_path(&child);
            if child_path == parent_path {
                break;
            }
            let base_name = tspath::get_base_file_name(&child);
            if let Some(dir_entry) = self.disk_directories.get_mut(&parent_path) {
                dir_entry.insert(child_path, base_name);
                break;
            }
            let mut dir = dirty::CloneableMap::new();
            dir.insert(child_path.clone(), base_name);
            self.disk_directories.insert(parent_path.clone(), dir);
            child_path = parent_path;
            child = parent;
        }
    }

    fn on_deleted_file_or_directory(&mut self, path: tspath::Path) {
        let parent = tspath::get_directory_path(&path);
        let Some(dir_entry) = self.disk_directories.get_mut(&parent) else {
            return;
        };
        dir_entry.remove(&path);
        if dir_entry.is_empty() {
            self.disk_directories.remove(&parent);
            self.on_deleted_file_or_directory(parent);
        }
    }

    pub fn is_open_file(&self, path: &tspath::Path) -> bool {
        self.overlays.contains_key(path)
    }

    pub fn get_file(&mut self, file_name: &str) -> Option<FileHandleRef> {
        <Self as FileSource>::get_file(self, file_name)
    }

    pub fn file_exists(&mut self, file_name: &str, path: &tspath::Path) -> bool {
        <Self as FileSource>::file_exists(self, file_name, path)
    }

    pub fn get_file_by_path(
        &mut self,
        file_name: &str,
        path: &tspath::Path,
    ) -> Option<FileHandleRef> {
        <Self as FileSource>::get_file_by_path(self, file_name, path)
    }

    pub fn get_accessible_entries(&mut self, path: &str) -> vfs::Entries {
        <Self as FileSource>::get_accessible_entries(self, path)
    }

    fn get_disk_file(
        &mut self,
        file_name: &str,
        path: &tspath::Path,
        force_reload: bool,
    ) -> Option<FileHandleRef> {
        let loaded = self.disk_files.load(path).is_some();
        if !loaded {
            let mut file = new_disk_file(file_name.to_owned(), String::new());
            file.needs_reload = true;
            let file = Arc::new(file);
            self.disk_files.store(path.clone(), file.clone());
            if path.contains("/node_modules/") {
                let file = self.record_realpath_alias(file, file_name, path);
                self.disk_files.store(path.clone(), file);
            }
        }
        if force_reload {
            return self
                .reload_entry_path(path)
                .map(|file| file as FileHandleRef);
        }
        self.reload_entry_if_needed_path(path)
            .map(|file| file as FileHandleRef)
    }

    // recordRealpathAlias checks if fileName is accessed through a symlink and, if so,
    // records a mapping from the realpath-based key to the symlink-based key.
    // This is only called for files inside node_modules where symlinks are common.
    fn record_realpath_alias(
        &mut self,
        disk_file: Arc<DiskFile>,
        symlink_file_name: &str,
        symlink_path: &tspath::Path,
    ) -> Arc<DiskFile> {
        let realpath = self.fs.realpath(symlink_file_name);
        let realpath_path = (self.to_path)(&realpath);
        if realpath_path == *symlink_path {
            return disk_file;
        }
        let mut file = disk_file.as_ref().clone();
        file.realpath_path = realpath_path.clone();
        self.node_modules_realpath_aliases
            .add(realpath_path, symlink_path.clone());
        Arc::new(file)
    }

    fn reload_entry(&self, entry: &Arc<DiskFile>) -> Option<Arc<DiskFile>> {
        let file_name = entry.file_name();
        if file_name.is_empty() {
            return None;
        }
        let (content, ok) = self.fs.read_file(&file_name);
        if ok {
            let mut file = new_disk_file(file_name, content);
            file.realpath_path = entry.realpath_path.clone();
            Some(Arc::new(file))
        } else {
            None
        }
    }

    fn reload_entry_path(&mut self, path: &tspath::Path) -> Option<Arc<DiskFile>> {
        let entry = self.disk_files.value(path)?;
        let reloaded = self.reload_entry(&entry);
        if let Some(file) = &reloaded {
            self.disk_files.store(path.clone(), file.clone());
        } else {
            self.disk_files.delete(path);
        }
        reloaded
    }

    pub(crate) fn reload_entry_if_needed(&self, entry: &Arc<DiskFile>) -> Option<Arc<DiskFile>> {
        if entry.matches_disk_text() {
            return Some(entry.clone());
        }
        self.reload_entry(entry)
    }

    fn reload_entry_if_needed_path(&mut self, path: &tspath::Path) -> Option<Arc<DiskFile>> {
        let entry = self.disk_files.value(path)?;
        if entry.matches_disk_text() {
            return Some(entry);
        }
        let reloaded = self.reload_entry(&entry);
        if let Some(file) = &reloaded {
            self.disk_files.store(path.clone(), file.clone());
        } else {
            self.disk_files.delete(path);
        }
        reloaded
    }

    pub fn watch_changes_overlap_cache(&self, change: &FileChangeSummary) -> bool {
        for uri in &change.changed {
            let path = (self.to_path)(&uri.file_name());
            if self.disk_files.load(&path).is_some()
                || self.node_modules_realpath_aliases.contains_key(&path)
            {
                return true;
            }
        }
        for uri in &change.deleted {
            let path = (self.to_path)(&uri.file_name());
            if self.disk_files.load(&path).is_some()
                || self.node_modules_realpath_aliases.contains_key(&path)
            {
                return true;
            }
        }
        false
    }

    pub fn invalidate_cache(&mut self) {
        for path in self.disk_files.keys() {
            let Some(file) = self.disk_files.value(&path) else {
                continue;
            };
            let mut next = file.as_ref().clone();
            next.needs_reload = true;
            self.disk_files.store(path, Arc::new(next));
        }
    }

    pub fn invalidate_node_modules_cache(&mut self) {
        for path in self.disk_files.keys() {
            if path.contains("/node_modules/") {
                if let Some(file) = self.disk_files.value(&path) {
                    let mut next = file.as_ref().clone();
                    next.needs_reload = true;
                    self.disk_files.store(path, Arc::new(next));
                }
            }
        }
    }

    pub fn mark_dirty_files(&mut self, change: &FileChangeSummary) {
        for uri in &change.changed {
            let path = (self.to_path)(&uri.file_name());
            if let Some(file) = self.disk_files.value(&path) {
                let mut next = file.as_ref().clone();
                next.needs_reload = true;
                self.disk_files.store(path, Arc::new(next));
            }
        }
        for uri in &change.deleted {
            let path = (self.to_path)(&uri.file_name());
            if self.disk_files.load(&path).is_some() {
                self.disk_files.delete(&path);
            }
        }
    }

    pub fn is_relevant_file_name(&self, uri: &lsproto::DocumentUri) -> bool {
        let file_name = uri.file_name();
        if tspath::is_dynamic_file_name(&file_name) {
            return true;
        }
        let path = (self.to_path)(&file_name);
        if self.overlays.contains_key(&path) {
            return true;
        }
        let Some(i) = path.rfind('.') else {
            return false;
        };
        matches!(
            &path[i..],
            ".js" | ".jsx" | ".mjs" | ".cjs" | ".ts" | ".tsx" | ".mts" | ".cts" | ".json"
        )
    }

    pub fn expand_and_filter_watch_events(
        &self,
        mut change: FileChangeSummary,
    ) -> FileChangeSummary {
        if !change.deleted.is_empty() {
            let mut filtered_deleted = HashSet::new();
            for uri in &change.deleted {
                let path = (self.to_path)(&uri.file_name());
                if self.disk_directories.contains_key(&path) {
                    self.collect_files_recursive(&path, &mut filtered_deleted);
                } else if self.is_relevant_file_name(uri) {
                    filtered_deleted.insert(uri.clone());
                }
            }
            change.deleted = filtered_deleted;
        }

        if !change.changed.is_empty() {
            let mut filtered_changed = HashSet::new();
            for uri in &change.changed {
                if self.is_relevant_file_name(uri) {
                    filtered_changed.insert(uri.clone());
                }
            }
            change.changed = filtered_changed;
        }

        change
    }

    pub fn collect_files_recursive(
        &self,
        dir_path: &tspath::Path,
        files: &mut HashSet<lsproto::DocumentUri>,
    ) {
        let Some(dir_entry) = self.disk_directories.get(dir_path) else {
            return;
        };
        for child_path in dir_entry.keys() {
            if let Some(file) = self.disk_files.value(child_path) {
                files.insert(lsconv::file_name_to_document_uri(&file.file_name()));
            }
            self.collect_files_recursive(child_path, files);
        }
    }

    pub fn convert_open_and_close_to_changes(
        &mut self,
        mut change: FileChangeSummary,
    ) -> FileChangeSummary {
        if !change.opened.is_empty() && !tspath::is_dynamic_file_name(&change.opened.file_name()) {
            let path = (self.to_path)(&change.opened.file_name());
            if self.disk_files.load(&path).is_none() || self.disk_files.original(&path).is_none() {
                change.created.insert(change.opened.clone());
            } else if let Some(overlay) = self.overlays.get(&path) {
                if let Some(disk_file) = self.disk_files.original(&path) {
                    if overlay.hash() != disk_file.hash() {
                        change.changed.insert(change.opened.clone());
                    }
                }
            }
        }
        let closed: Vec<_> = change.closed.iter().cloned().collect();
        for uri in closed {
            let file_name = uri.file_name();
            if tspath::is_dynamic_file_name(&file_name) {
                continue;
            }
            let path = (self.to_path)(&file_name);
            if let Some(file) = self.get_disk_file(&file_name, &path, true) {
                if let Some(prev_overlay) = self.prev_overlays.get(&path) {
                    if file.hash() != prev_overlay.hash() {
                        change.changed.insert(uri);
                    }
                }
                continue;
            }
            change.deleted.insert(uri);
        }
        change
    }
}

// sourceFS is a vfs.FS that sources files from a FileSource and tracks seen files.
pub struct SourceFs {
    tracking: AtomicBool,
    to_path: ToPath,
    missing_directories: RwLock<Option<collections::SyncSet<tspath::Path>>>,
    seen_files: RwLock<Option<collections::SyncSet<tspath::Path>>>,
    source: Arc<Mutex<Box<dyn FileSource>>>,
}

pub fn new_source_fs<S>(tracking: bool, source: S, to_path: ToPath) -> SourceFs
where
    S: FileSource + 'static,
{
    SourceFs {
        tracking: AtomicBool::new(tracking),
        to_path,
        seen_files: RwLock::new(tracking.then(collections::SyncSet::new)),
        missing_directories: RwLock::new(tracking.then(collections::SyncSet::new)),
        source: Arc::new(Mutex::new(Box::new(source))),
    }
}

impl Clone for SourceFs {
    fn clone(&self) -> Self {
        Self {
            tracking: AtomicBool::new(self.tracking.load(Ordering::SeqCst)),
            to_path: self.to_path.clone(),
            missing_directories: RwLock::new(
                self.missing_directories
                    .read()
                    .unwrap_or_else(|err| err.into_inner())
                    .clone(),
            ),
            seen_files: RwLock::new(
                self.seen_files
                    .read()
                    .unwrap_or_else(|err| err.into_inner())
                    .clone(),
            ),
            source: Arc::clone(&self.source),
        }
    }
}

impl SourceFs {
    pub fn fs(&self) -> Arc<dyn vfs::Fs + Send + Sync> {
        self.source
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .fs()
    }

    pub fn to_path(&self, file_name: &str) -> tspath::Path {
        (self.to_path)(file_name)
    }

    pub fn disable_tracking(&self) {
        self.tracking.store(false, Ordering::SeqCst);
    }

    pub(crate) fn set_source<S>(&self, source: S)
    where
        S: FileSource + 'static,
    {
        *self.source.lock().unwrap_or_else(|err| err.into_inner()) = Box::new(source);
    }

    pub fn track(&self, file_name: &str) {
        if !self.tracking.load(Ordering::SeqCst) {
            return;
        }
        if let Some(seen_files) = &*self
            .seen_files
            .read()
            .unwrap_or_else(|err| err.into_inner())
        {
            seen_files.add((self.to_path)(file_name));
        }
    }

    pub fn seen_file(&self, path: &tspath::Path) -> bool {
        self.seen_files
            .read()
            .unwrap_or_else(|err| err.into_inner())
            .as_ref()
            .is_some_and(|seen_files| seen_files.has(path))
    }

    pub fn seen_files(&self) -> Option<collections::SyncSet<tspath::Path>> {
        self.seen_files
            .read()
            .unwrap_or_else(|err| err.into_inner())
            .clone()
    }

    pub fn set_seen_files(&self, seen_files: Option<collections::SyncSet<tspath::Path>>) {
        *self
            .seen_files
            .write()
            .unwrap_or_else(|err| err.into_inner()) = seen_files;
    }

    pub fn seen_file_or_missing_parent_directory(&self, mut path: tspath::Path) -> bool {
        if self
            .seen_files
            .read()
            .unwrap_or_else(|err| err.into_inner())
            .as_ref()
            .is_some_and(|seen_files| seen_files.has(&path))
        {
            return true;
        }
        if let Some(missing_directories) = &*self
            .missing_directories
            .read()
            .unwrap_or_else(|err| err.into_inner())
        {
            if !missing_directories.is_empty() {
                loop {
                    if missing_directories.has(&path) {
                        return true;
                    }
                    let parent = tspath::get_directory_path(&path);
                    if parent == path {
                        break;
                    }
                    path = parent;
                }
            }
        }
        false
    }

    pub fn get_file(&self, file_name: &str) -> Option<FileHandleRef> {
        self.track(file_name);
        self.source
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .get_file(file_name)
    }

    pub fn get_file_by_path(&self, file_name: &str, path: &tspath::Path) -> Option<FileHandleRef> {
        self.track(file_name);
        self.source
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .get_file_by_path(file_name, path)
    }
}

impl vfs::Fs for SourceFs {
    fn use_case_sensitive_file_names(&self) -> bool {
        self.source
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .fs()
            .use_case_sensitive_file_names()
    }

    fn file_exists(&self, path: &str) -> bool {
        self.track(path);
        self.source
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .file_exists(path, &(self.to_path)(path))
    }

    fn read_file(&self, path: &str) -> (String, bool) {
        if let Some(file) = self.get_file(path) {
            return (file.content(), true);
        }
        (String::new(), false)
    }

    fn write_file(&self, path: &str, data: &str) -> io::Result<()> {
        self.source
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .fs()
            .write_file(path, data)
    }

    fn append_file(&self, path: &str, data: &str) -> io::Result<()> {
        self.source
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .fs()
            .append_file(path, data)
    }

    fn remove(&self, path: &str) -> io::Result<()> {
        self.source
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .fs()
            .remove(path)
    }

    fn chtimes(&self, path: &str, atime: SystemTime, mtime: SystemTime) -> io::Result<()> {
        self.source
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .fs()
            .chtimes(path, atime, mtime)
    }

    fn directory_exists(&self, path: &str) -> bool {
        let exists = self
            .source
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .fs()
            .directory_exists(path);
        if !exists && self.tracking.load(Ordering::SeqCst) {
            if let Some(missing_directories) = &*self
                .missing_directories
                .read()
                .unwrap_or_else(|err| err.into_inner())
            {
                missing_directories.add((self.to_path)(path));
            }
        }
        exists
    }

    fn get_accessible_entries(&self, path: &str) -> vfs::Entries {
        self.source
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .get_accessible_entries(path)
    }

    fn stat(&self, path: &str) -> io::Result<vfs::FileInfo> {
        self.source
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .fs()
            .stat(path)
    }

    fn walk_dir(&self, root: &str, walk_fn: &mut vfs::WalkDirFunc<'_>) -> io::Result<()> {
        self.source
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .fs()
            .walk_dir(root, walk_fn)
    }

    fn realpath(&self, path: &str) -> String {
        self.source
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .fs()
            .realpath(path)
    }
}

pub fn read_directory_into_entries<M>(
    directories: &M,
    is_file: impl Fn(&tspath::Path) -> bool,
    entries: &mut vfs::Entries,
) where
    for<'a> &'a M: IntoIterator<Item = (&'a tspath::Path, &'a String)>,
{
    for (child_path, child_name) in directories {
        if is_file(child_path) {
            entries.files.push(child_name.clone());
        } else {
            entries.directories.push(child_name.clone());
        }
    }
}
