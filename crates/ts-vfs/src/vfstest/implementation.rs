use std::collections::{BTreeMap, BTreeSet};
use std::io;
use std::sync::{Arc, RwLock};
use std::time::{Duration, SystemTime};

use crate::vfs::{DirEntry, Entries, FileInfo, FileType, Fs};

pub trait Clock: Send + Sync {
    fn now(&self) -> SystemTime;
    fn since_start(&self) -> Duration;
}

#[derive(Clone, Debug)]
pub struct ClockImpl {
    start: SystemTime,
}

impl Default for ClockImpl {
    fn default() -> Self {
        Self {
            start: SystemTime::now(),
        }
    }
}

impl Clock for ClockImpl {
    fn now(&self) -> SystemTime {
        SystemTime::now()
    }

    fn since_start(&self) -> Duration {
        self.start.elapsed().unwrap_or_default()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MapFile {
    pub data: Arc<[u8]>,
    pub text: Option<Arc<str>>,
    pub mode: FileType,
    pub mod_time: SystemTime,
    pub realpath: String,
}

impl Default for MapFile {
    fn default() -> Self {
        Self {
            data: Arc::from([]),
            text: None,
            mode: FileType::file(),
            mod_time: SystemTime::UNIX_EPOCH,
            realpath: String::new(),
        }
    }
}

#[derive(Clone)]
pub struct MapFs {
    inner: Arc<RwLock<MapFsInner>>,
    clock: Arc<dyn Clock>,
}

#[derive(Clone, Default)]
struct MapFsInner {
    files: BTreeMap<String, MapFile>,
    symlinks: BTreeMap<String, String>,
    use_case_sensitive_file_names: bool,
}

pub trait IntoMapFile {
    fn into_map_file(self, mod_time: SystemTime) -> MapFile;
}

impl IntoMapFile for String {
    fn into_map_file(self, mod_time: SystemTime) -> MapFile {
        let text = Arc::<str>::from(self);
        MapFile {
            data: Arc::from(text.as_bytes()),
            text: Some(text),
            mode: FileType::file(),
            mod_time,
            realpath: String::new(),
        }
    }
}

impl IntoMapFile for &str {
    fn into_map_file(self, mod_time: SystemTime) -> MapFile {
        self.to_owned().into_map_file(mod_time)
    }
}

impl IntoMapFile for Vec<u8> {
    fn into_map_file(self, mod_time: SystemTime) -> MapFile {
        MapFile {
            data: self.into(),
            text: None,
            mode: FileType::file(),
            mod_time,
            realpath: String::new(),
        }
    }
}

impl IntoMapFile for &[u8] {
    fn into_map_file(self, mod_time: SystemTime) -> MapFile {
        MapFile {
            data: Arc::from(self),
            text: None,
            mode: FileType::file(),
            mod_time,
            realpath: String::new(),
        }
    }
}

impl IntoMapFile for MapFile {
    fn into_map_file(mut self, mod_time: SystemTime) -> MapFile {
        self.mod_time = mod_time;
        self
    }
}

pub fn from_map<I, P, F>(files: I, use_case_sensitive_file_names: bool) -> MapFs
where
    I: IntoIterator<Item = (P, F)>,
    P: Into<String>,
    F: IntoMapFile,
{
    let clock: Arc<dyn Clock> = Arc::new(ClockImpl::default());
    from_map_with_clock(files, use_case_sensitive_file_names, clock)
}

pub fn from_map_with_clock<I, P, F>(
    files: I,
    use_case_sensitive_file_names: bool,
    clock: Arc<dyn Clock>,
) -> MapFs
where
    I: IntoIterator<Item = (P, F)>,
    P: Into<String>,
    F: IntoMapFile,
{
    let mut posix = false;
    let mut windows = false;
    let mut entries = files
        .into_iter()
        .map(|(path, file)| {
            let path = path.into();
            validate_rooted_normalized_path(&path);
            if path.starts_with('/') {
                posix = true;
            } else {
                windows = true;
            }
            (path, file)
        })
        .collect::<Vec<_>>();
    entries.sort_by(|(a, _), (b, _)| compare_paths_by_parts(a, b));

    if posix && windows {
        panic!("mixed posix and windows paths");
    }

    let fs = MapFs {
        inner: Arc::new(RwLock::new(MapFsInner {
            files: BTreeMap::new(),
            symlinks: BTreeMap::new(),
            use_case_sensitive_file_names,
        })),
        clock,
    };

    let mut canonical_paths = BTreeMap::<String, String>::new();
    for (path, _) in &entries {
        let canonical_path = canonical(path, use_case_sensitive_file_names);
        if let Some(other) = canonical_paths.insert(canonical_path, path.clone()) {
            let (first, second) = if path <= &other {
                (path, &other)
            } else {
                (&other, path)
            };
            panic!(
                "duplicate path: {:?} and {:?} have the same canonical path",
                first, second
            );
        }
    }

    for (path, data) in entries {
        let mut file = data.into_map_file(fs.clock.now());
        if file.mode.is_symlink() {
            let target = std::str::from_utf8(&file.data).unwrap_or_default();
            validate_rooted_normalized_path(&target);
        }
        if file.realpath.is_empty() {
            file.realpath = path.clone();
        }
        fs.set_entry(&path, file);
    }
    fs
}

pub fn symlink(target: impl Into<String>) -> MapFile {
    let target = target.into();
    MapFile {
        data: target.into_bytes().into(),
        text: None,
        mode: FileType::symlink(),
        mod_time: SystemTime::UNIX_EPOCH,
        realpath: String::new(),
    }
}

impl MapFs {
    fn normalize_to_existing_root_style(&self, path: &str) -> String {
        if path.starts_with('/') || is_windows_rooted(path) {
            return path.to_owned();
        }
        let inner = self.inner.read().unwrap();
        if inner.files.keys().any(|key| key.starts_with('/')) {
            format!("/{path}")
        } else {
            path.to_owned()
        }
    }

    pub fn add_symlink(&self, path: &str, target: &str) {
        let path = self.normalize_to_existing_root_style(path);
        let target = self.normalize_to_existing_root_style(target);
        let canonical = self.get_canonical_path(&path);
        self.set_entry_at(
            &path,
            canonical,
            MapFile {
                data: Arc::from(target.as_bytes()),
                text: None,
                mode: FileType::symlink(),
                mod_time: self.clock.now(),
                realpath: path.clone(),
            },
        );
    }

    pub fn get_canonical_path(&self, path: &str) -> String {
        let inner = self.inner.read().unwrap();
        canonical(path, inner.use_case_sensitive_file_names)
    }

    pub fn set(&self, path: &str, mut file: MapFile) {
        if file.realpath.is_empty() {
            file.realpath = path.to_owned();
        }
        self.set_entry(path, file);
    }

    pub fn mkdir_all(&self, path: &str) {
        let path = self.normalize_to_existing_root_style(path);
        let _ = self.mkdir_all_inner(&path);
    }

    pub fn get_target_of_symlink(&self, path: &str) -> Option<String> {
        let inner = self.inner.read().unwrap();
        let canonical = canonical(path, inner.use_case_sensitive_file_names);
        inner
            .files
            .get(&canonical)
            .filter(|file| file.mode.is_symlink())
            .and_then(|file| std::str::from_utf8(&file.data).ok().map(str::to_owned))
            .map(|target| {
                if target.starts_with('/') || is_windows_rooted(&target) {
                    target
                } else {
                    format!("/{target}")
                }
            })
    }

    pub fn get_mod_time(&self, path: &str) -> Option<SystemTime> {
        let inner = self.inner.read().unwrap();
        let canonical = canonical(path, inner.use_case_sensitive_file_names);
        inner.files.get(&canonical).map(|file| file.mod_time)
    }

    pub fn get_file_info(&self, path: &str) -> Option<MapFile> {
        let inner = self.inner.read().unwrap();
        let canonical = canonical(path, inner.use_case_sensitive_file_names);
        inner.files.get(&canonical).cloned()
    }

    pub fn entries(&self) -> Vec<(String, MapFile)> {
        let inner = self.inner.read().unwrap();
        let mut keys = inner.files.keys().cloned().collect::<Vec<_>>();
        keys.sort_by(|a, b| compare_paths_by_parts(a, b));
        keys.into_iter()
            .filter_map(|canonical_path| {
                inner.files.get(&canonical_path).cloned().map(|file| {
                    let path =
                        if file.realpath.starts_with('/') || is_windows_rooted(&file.realpath) {
                            file.realpath.clone()
                        } else {
                            format!("/{}", file.realpath)
                        };
                    (path, file)
                })
            })
            .collect()
    }

    fn set_entry(&self, path: &str, file: MapFile) {
        let canonical_path = {
            let inner = self.inner.read().unwrap();
            canonical(path, inner.use_case_sensitive_file_names)
        };
        self.set_entry_at(path, canonical_path, file);
    }

    fn set_entry_at(&self, path: &str, canonical_path: String, mut file: MapFile) {
        if path.is_empty() || canonical_path.is_empty() {
            panic!("empty path");
        }

        let mut inner = self.inner.write().unwrap();
        if file.realpath.is_empty() {
            file.realpath = path.to_owned();
        }
        let symlink_target = if file.mode.is_symlink() {
            Some(canonical(
                &symlink_target_text(&file),
                inner.use_case_sensitive_file_names,
            ))
        } else {
            None
        };
        let mut dirs = parent_dirs(path);
        dirs.sort_by(|a, b| compare_paths_by_parts(a, b));
        for dir in dirs {
            let dir_canonical = canonical(&dir, inner.use_case_sensitive_file_names);
            inner.files.entry(dir_canonical).or_insert_with(|| MapFile {
                data: Arc::from([]),
                text: None,
                mode: FileType::directory(),
                mod_time: self.clock.now(),
                realpath: dir,
            });
        }
        if let Some(target) = symlink_target {
            inner.symlinks.insert(canonical_path.clone(), target);
        } else {
            inner.symlinks.remove(&canonical_path);
        }
        inner.files.insert(canonical_path, file);
    }

    fn insert_entry_at(&self, path: &str, canonical_path: String, mut file: MapFile) {
        if path.is_empty() || canonical_path.is_empty() {
            panic!("empty path");
        }

        let mut inner = self.inner.write().unwrap();
        if file.realpath.is_empty() {
            file.realpath = path.to_owned();
        }
        if file.mode.is_symlink() {
            let target = canonical(
                &symlink_target_text(&file),
                inner.use_case_sensitive_file_names,
            );
            inner.symlinks.insert(canonical_path.clone(), target);
        } else {
            inner.symlinks.remove(&canonical_path);
        }
        inner.files.insert(canonical_path, file);
    }

    fn mkdir_all_inner(&self, path: &str) -> io::Result<()> {
        if path.is_empty() {
            panic!("empty path");
        }

        if let Ok(Some((_, file))) = self.resolve(path) {
            if file.mode.is_dir() {
                return Ok(());
            }
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                format!("mkdir {path:?}: path exists but is not a directory"),
            ));
        }

        let mut to_create = Vec::new();
        let mut p = path.to_owned();
        let mut offset = rooted_path_offset(&p);
        loop {
            let (dir, rest) = split_path(&p, offset);
            let canonical = self.get_canonical_path(&dir);
            match self.resolve(&dir) {
                Ok(Some((other_path, other))) => {
                    if !other.mode.is_dir() {
                        return Err(io::Error::new(
                            io::ErrorKind::AlreadyExists,
                            format!("mkdir {other_path:?}: path exists but is not a directory"),
                        ));
                    }
                    if canonical != other_path {
                        p = if rest.is_empty() {
                            other.realpath
                        } else {
                            format!("{}/{}", other.realpath.trim_end_matches('/'), rest)
                        };
                        to_create.clear();
                        offset = rooted_path_offset(&p);
                        continue;
                    }
                }
                Ok(None) => to_create.push(dir.clone()),
                Err(err) if err.kind() == io::ErrorKind::NotFound => to_create.push(dir.clone()),
                Err(err) => return Err(err),
            }
            if rest.is_empty() {
                break;
            }
            offset = dir.len() + 1;
        }

        for dir in to_create {
            self.set_entry(
                &dir,
                MapFile {
                    data: Arc::from([]),
                    text: None,
                    mode: FileType::directory(),
                    mod_time: self.clock.now(),
                    realpath: dir.clone(),
                },
            );
        }
        Ok(())
    }

    fn resolve(&self, path: &str) -> io::Result<Option<(String, MapFile)>> {
        let inner = self.inner.read().unwrap();
        inner.resolve(path)
    }

    fn exact_symlink_target(&self, path: &str) -> Option<String> {
        let inner = self.inner.read().unwrap();
        inner.exact_symlink_target(path)
    }

    fn translate_symlink_path(&self, path: &str) -> String {
        let inner = self.inner.read().unwrap();
        inner.translate_symlink_path(path)
    }
}

impl MapFsInner {
    fn get_canonical_path(&self, path: &str) -> String {
        canonical(path, self.use_case_sensitive_file_names)
    }

    fn resolve(&self, path: &str) -> io::Result<Option<(String, MapFile)>> {
        self.resolve_worker(path, 0, None)
    }

    fn resolve_worker(
        &self,
        path: &str,
        depth: usize,
        broken_from: Option<(String, String)>,
    ) -> io::Result<Option<(String, MapFile)>> {
        if depth > 40 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("too many symlinks resolving {path:?}"),
            ));
        }

        let canonical_path = self.get_canonical_path(path);
        if let Some(file) = self.files.get(&canonical_path) {
            if !file.mode.is_symlink() {
                return Ok(Some((canonical_path, file.clone())));
            }
            if let Some(target) = self.symlinks.get(&canonical_path) {
                return self.resolve_worker(
                    target,
                    depth + 1,
                    Some((canonical_path.clone(), target.clone())),
                );
            }
        }

        for (link_path, target) in &self.symlinks {
            let prefix = format!("{link_path}/");
            if canonical_path.starts_with(&prefix) {
                let rest = &canonical_path[prefix.len()..];
                let target_path = format!("{}/{}", target.trim_end_matches('/'), rest);
                return self.resolve_worker(
                    &target_path,
                    depth + 1,
                    Some((link_path.clone(), target.clone())),
                );
            }
        }

        if let Some((from, to)) = broken_from {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!(
                    "broken symlink {:?} -> {:?}",
                    display_error_path(&from),
                    display_error_path(&to)
                ),
            ));
        }
        Ok(None)
    }

    fn exact_symlink_target(&self, path: &str) -> Option<String> {
        self.exact_symlink_target_worker(path, 0)
    }

    fn exact_symlink_target_worker(&self, path: &str, depth: usize) -> Option<String> {
        if depth > 40 {
            return None;
        }

        let canonical_path = self.get_canonical_path(path);
        if let Some(file) = self.files.get(&canonical_path) {
            if !file.mode.is_symlink() {
                return Some(canonical_path);
            }
            if let Some(target) = self.symlinks.get(&canonical_path) {
                return self
                    .exact_symlink_target_worker(target, depth + 1)
                    .or_else(|| Some(target.clone()));
            }
        }

        for (link_path, target) in &self.symlinks {
            let prefix = format!("{link_path}/");
            if canonical_path.starts_with(&prefix) {
                let rest = &canonical_path[prefix.len()..];
                let target_path = format!("{}/{}", target.trim_end_matches('/'), rest);
                return self
                    .exact_symlink_target_worker(&target_path, depth + 1)
                    .or_else(|| Some(self.get_canonical_path(&target_path)));
            }
        }
        None
    }

    fn translate_symlink_path(&self, path: &str) -> String {
        let mut current = path.to_owned();
        for _ in 0..40 {
            let canonical_path = self.get_canonical_path(&current);
            if let Some(target) = self.symlinks.get(&canonical_path) {
                current = target.clone();
                continue;
            }
            let mut translated = None;
            for (link_path, target) in &self.symlinks {
                let prefix = format!("{link_path}/");
                if canonical_path.starts_with(&prefix) {
                    let rest = &canonical_path[prefix.len()..];
                    translated = Some(format!("{}/{}", target.trim_end_matches('/'), rest));
                    break;
                }
            }
            match translated {
                Some(path) => current = path,
                None => return current,
            }
        }
        current
    }
}

impl MapFs {
    pub fn read_file_text(&self, path: &str) -> (Arc<str>, bool) {
        let Some((_, file)) = self.resolve(path).ok().flatten() else {
            return (Arc::<str>::from(""), false);
        };
        if !file.mode.is_file() {
            return (Arc::<str>::from(""), false);
        }
        if let Some(text) = file.text {
            return (text, true);
        }
        decode_bytes(&file.data)
            .map(|text| (Arc::<str>::from(text), true))
            .unwrap_or_else(|| (Arc::<str>::from(""), false))
    }
}

impl Fs for MapFs {
    fn use_case_sensitive_file_names(&self) -> bool {
        self.inner.read().unwrap().use_case_sensitive_file_names
    }

    fn file_exists(&self, path: &str) -> bool {
        self.resolve(path)
            .ok()
            .flatten()
            .is_some_and(|(_, file)| file.mode.is_file())
    }

    fn read_file(&self, path: &str) -> (String, bool) {
        let Some((_, file)) = self.resolve(path).ok().flatten() else {
            return (String::new(), false);
        };
        if !file.mode.is_file() {
            return (String::new(), false);
        }
        if let Some(text) = file.text {
            return (text.to_string(), true);
        }
        decode_bytes(&file.data)
            .map(|text| (text, true))
            .unwrap_or_else(|| (String::new(), false))
    }

    fn write_file(&self, path: &str, data: &str) -> io::Result<()> {
        if let Some(parent) = dir_name(path) {
            match self.resolve(&parent) {
                Ok(Some((_, parent_file))) => {
                    if !parent_file.mode.is_dir() {
                        return Err(io::Error::new(
                            io::ErrorKind::AlreadyExists,
                            format!("write {path:?}: parent path exists but is not a directory"),
                        ));
                    }
                }
                Ok(None) => {
                    self.mkdir_all_inner(&parent)?;
                }
                Err(_) => self.mkdir_all_inner(&parent)?,
            }
        }
        let canonical_target = match self.resolve(path) {
            Ok(Some((canonical_target, existing))) => {
                if !existing.mode.is_file() {
                    return Err(io::Error::new(
                        io::ErrorKind::AlreadyExists,
                        format!("write {path:?}: path exists but is not a regular file"),
                    ));
                }
                canonical_target
            }
            Ok(None) => self
                .exact_symlink_target(path)
                .unwrap_or_else(|| self.get_canonical_path(&self.translate_symlink_path(path))),
            Err(err) => {
                let Some(canonical_target) = self.exact_symlink_target(path) else {
                    return Err(io::Error::new(err.kind(), format!("write {path:?}: {err}")));
                };
                canonical_target
            }
        };
        self.insert_entry_at(
            path,
            canonical_target,
            MapFile {
                data: Arc::from(data.as_bytes()),
                text: Some(Arc::<str>::from(data)),
                mode: FileType::file(),
                mod_time: self.clock.now(),
                realpath: path.to_owned(),
            },
        );
        Ok(())
    }

    fn append_file(&self, path: &str, data: &str) -> io::Result<()> {
        if let Some(parent) = dir_name(path) {
            match self.resolve(&parent) {
                Ok(Some((_, parent_file))) => {
                    if !parent_file.mode.is_dir() {
                        return Err(io::Error::new(
                            io::ErrorKind::AlreadyExists,
                            format!("append {path:?}: parent path exists but is not a directory"),
                        ));
                    }
                }
                Ok(None) => {
                    self.mkdir_all_inner(&parent)?;
                }
                Err(_) => self.mkdir_all_inner(&parent)?,
            }
        }
        let mut existing = Vec::new();
        let canonical_target = match self.resolve(path) {
            Ok(Some((canonical_target, file))) => {
                if !file.mode.is_file() {
                    return Err(io::Error::new(
                        io::ErrorKind::AlreadyExists,
                        format!("append {path:?}: path exists but is not a regular file"),
                    ));
                }
                existing = file.data.to_vec();
                canonical_target
            }
            Ok(None) => self
                .exact_symlink_target(path)
                .unwrap_or_else(|| self.get_canonical_path(&self.translate_symlink_path(path))),
            Err(err) => {
                let Some(canonical_target) = self.exact_symlink_target(path) else {
                    return Err(io::Error::new(
                        err.kind(),
                        format!("append {path:?}: {err}"),
                    ));
                };
                canonical_target
            }
        };
        existing.extend_from_slice(data.as_bytes());
        let text = decode_bytes(&existing).map(Arc::<str>::from);
        self.insert_entry_at(
            path,
            canonical_target,
            MapFile {
                data: existing.into(),
                text,
                mode: FileType::file(),
                mod_time: self.clock.now(),
                realpath: path.to_owned(),
            },
        );
        Ok(())
    }

    fn remove(&self, path: &str) -> io::Result<()> {
        let mut inner = self.inner.write().unwrap();
        let canonical = canonical(path, inner.use_case_sensitive_file_names);
        inner.files.remove(&canonical);
        inner.symlinks.remove(&canonical);
        let prefix = format!("{canonical}/");
        inner.files.retain(|key, _| !key.starts_with(&prefix));
        inner.symlinks.retain(|key, _| !key.starts_with(&prefix));
        Ok(())
    }

    fn chtimes(&self, path: &str, _atime: SystemTime, mtime: SystemTime) -> io::Result<()> {
        let mut inner = self.inner.write().unwrap();
        let canonical = canonical(path, inner.use_case_sensitive_file_names);
        let Some(file) = inner.files.get_mut(&canonical) else {
            return Err(io::ErrorKind::NotFound.into());
        };
        file.mod_time = mtime;
        Ok(())
    }

    fn directory_exists(&self, path: &str) -> bool {
        self.resolve(path)
            .ok()
            .flatten()
            .is_some_and(|(_, file)| file.mode.is_dir())
    }

    fn get_accessible_entries(&self, path: &str) -> Entries {
        let Some((canonical_path, directory)) = self.resolve(path).ok().flatten() else {
            return Entries {
                files: Vec::new(),
                directories: Vec::new(),
                symlinks: Some(Default::default()),
            };
        };
        if !directory.mode.is_dir() {
            return Entries {
                files: Vec::new(),
                directories: Vec::new(),
                symlinks: Some(Default::default()),
            };
        }

        let inner = self.inner.read().unwrap();
        let prefix = if canonical_path == "/" {
            "/".to_owned()
        } else {
            format!("{}/", canonical_path.trim_end_matches('/'))
        };
        let real_prefix = if directory.realpath == "/" {
            "/".to_owned()
        } else {
            format!("{}/", directory.realpath.trim_end_matches('/'))
        };
        let mut files = BTreeSet::new();
        let mut directories = BTreeSet::new();
        let mut symlinks = BTreeSet::new();
        for (key, file) in &inner.files {
            if !key.starts_with(&prefix) {
                continue;
            }
            let canonical_rest = &key[prefix.len()..];
            let rest = file
                .realpath
                .strip_prefix(&real_prefix)
                .unwrap_or(canonical_rest);
            if rest.is_empty() {
                continue;
            }
            let is_dir = file.mode.is_dir()
                || (file.mode.is_symlink()
                    && inner
                        .resolve_worker(key, 0, None)
                        .ok()
                        .flatten()
                        .is_some_and(|(_, target)| target.mode.is_dir()));
            if let Some((dir, _)) = rest.split_once('/') {
                directories.insert(dir.to_owned());
            } else if is_dir {
                directories.insert(rest.to_owned());
            } else {
                files.insert(rest.to_owned());
            }
            if file.mode.is_symlink() {
                symlinks.insert(rest.split('/').next().unwrap_or(rest).to_owned());
            }
        }
        Entries {
            files: files.into_iter().collect(),
            directories: directories.into_iter().collect(),
            symlinks: Some(symlinks.into_iter().collect()),
        }
    }

    fn stat(&self, path: &str) -> io::Result<FileInfo> {
        let Some((_, file)) = self.resolve(path)? else {
            return Err(io::ErrorKind::NotFound.into());
        };
        let name = base_name(&file.realpath).unwrap_or_else(|| file.realpath.clone());
        Ok(
            FileInfo::new(name, file.data.len() as u64, file.mode, Some(file.mod_time))
                .with_realpath(file.realpath),
        )
    }

    fn walk_dir(&self, root: &str, walk_fn: &mut crate::WalkDirFunc<'_>) -> io::Result<()> {
        let mut dirs = vec![root.to_owned()];
        while let Some(dir) = dirs.pop() {
            let entries = self.get_accessible_entries(&dir);
            for child in entries.directories {
                let path = join_path(&dir, &child);
                walk_fn(&path, DirEntry::directory(child), None)?;
                dirs.push(path);
            }
            for child in entries.files {
                let path = join_path(&dir, &child);
                walk_fn(&path, DirEntry::file(child), None)?;
            }
        }
        Ok(())
    }

    fn realpath(&self, path: &str) -> String {
        self.resolve(path)
            .ok()
            .flatten()
            .map(|(_, file)| file.realpath)
            .unwrap_or_else(|| path.to_owned())
    }
}

fn validate_rooted_normalized_path(path: &str) {
    if !(path.starts_with('/') || is_windows_rooted(path)) {
        panic!("non-rooted path {path:?}");
    }
    if path.ends_with('/') || path.contains("/../") || path.contains("/./") {
        panic!("non-normalized path {path:?}");
    }
}

fn is_windows_rooted(path: &str) -> bool {
    path.len() >= 3 && path.as_bytes()[1] == b':' && path.as_bytes()[2] == b'/'
}

fn rooted_path_offset(path: &str) -> usize {
    if path.starts_with('/') {
        1
    } else if is_windows_rooted(path) {
        3
    } else {
        0
    }
}

fn display_error_path(path: &str) -> &str {
    path.strip_prefix('/').unwrap_or(path)
}

fn symlink_target_text(file: &MapFile) -> String {
    std::str::from_utf8(&file.data)
        .unwrap_or_default()
        .to_owned()
}

fn canonical(path: &str, case_sensitive: bool) -> String {
    let path = if rooted_path_offset(path) == path.len() {
        path.to_owned()
    } else {
        path.trim_end_matches('/').to_owned()
    };
    if case_sensitive {
        path
    } else {
        path.to_ascii_lowercase()
    }
}

fn parent_dirs(path: &str) -> Vec<String> {
    let mut result = Vec::new();
    let root_offset = rooted_path_offset(path);
    if root_offset > 0 {
        result.push(path[..root_offset].to_owned());
    }
    let mut current = path.trim_end_matches('/').to_owned();
    while let Some(parent) = dir_name(&current) {
        if parent.is_empty() || parent == current {
            break;
        }
        result.push(parent.clone());
        current = parent;
    }
    result
}

fn split_path(s: &str, offset: usize) -> (String, String) {
    if let Some(idx) = s[offset..].find('/') {
        let idx = idx + offset;
        (s[..idx].to_owned(), s[idx + 1..].to_owned())
    } else {
        (s.to_owned(), String::new())
    }
}

fn dir_name(path: &str) -> Option<String> {
    path.rsplit_once('/')
        .map(|(dir, _)| dir.to_owned())
        .filter(|dir| !dir.is_empty())
}

fn base_name(path: &str) -> Option<String> {
    path.rsplit('/').next().map(str::to_owned)
}

fn join_path(dir: &str, name: &str) -> String {
    format!("{}/{}", dir.trim_end_matches('/'), name)
}

pub fn compare_paths_by_parts(a: &str, b: &str) -> std::cmp::Ordering {
    let mut a_parts = a.split('/');
    let mut b_parts = b.split('/');
    loop {
        match (a_parts.next(), b_parts.next()) {
            (Some(a), Some(b)) if a != b => return a.cmp(b),
            (Some(_), Some(_)) => continue,
            (None, None) => return a.cmp(b),
            (None, Some(_)) => return std::cmp::Ordering::Less,
            (Some(_), None) => return std::cmp::Ordering::Greater,
        }
    }
}

pub fn decode_bytes(bytes: &[u8]) -> Option<String> {
    if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        return String::from_utf8(bytes[3..].to_vec()).ok();
    }
    if bytes.starts_with(&[0xFF, 0xFE]) {
        return decode_utf16(&bytes[2..], true);
    }
    if bytes.starts_with(&[0xFE, 0xFF]) {
        return decode_utf16(&bytes[2..], false);
    }
    String::from_utf8(bytes.to_vec()).ok()
}

fn decode_utf16(bytes: &[u8], little_endian: bool) -> Option<String> {
    let words = bytes
        .chunks_exact(2)
        .map(|chunk| {
            if little_endian {
                u16::from_le_bytes([chunk[0], chunk[1]])
            } else {
                u16::from_be_bytes([chunk[0], chunk[1]])
            }
        })
        .collect::<Vec<_>>();
    String::from_utf16(&words).ok()
}
