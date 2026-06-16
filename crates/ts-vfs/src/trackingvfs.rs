// Package trackingvfs provides a VFS wrapper that records every file path
// accessed during compilation. This allows watch mode to know exactly which
// files and directories the compiler depended on, including non-existent
// paths from failed module resolution.

use std::{collections::HashSet, io, sync::Mutex, time::SystemTime};

use crate::vfs::{DirEntry, Entries, FileInfo, Fs};

// FS wraps a vfs.FS and records every path accessed via read-like operations.
// Write operations (WriteFile, Remove, Chtimes) are not tracked since they
// represent outputs, not dependencies.
pub struct TrackingFs<F: Fs> {
    pub inner: F,
    pub seen_files: Mutex<HashSet<String>>,
}

impl<F: Fs> TrackingFs<F> {
    pub fn new(inner: F) -> TrackingFs<F> {
        TrackingFs {
            inner,
            seen_files: Mutex::new(HashSet::new()),
        }
    }

    fn add_seen(&self, path: &str) {
        self.seen_files
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .insert(path.to_owned());
    }
}

impl<F: Fs> Fs for TrackingFs<F> {
    fn read_file(&self, path: &str) -> (String, bool) {
        self.add_seen(path);
        self.inner.read_file(path)
    }

    fn file_exists(&self, path: &str) -> bool {
        self.add_seen(path);
        self.inner.file_exists(path)
    }

    fn use_case_sensitive_file_names(&self) -> bool {
        self.inner.use_case_sensitive_file_names()
    }

    fn write_file(&self, path: &str, data: &str) -> io::Result<()> {
        self.inner.write_file(path, data)
    }

    fn append_file(&self, path: &str, data: &str) -> io::Result<()> {
        self.inner.append_file(path, data)
    }

    fn remove(&self, path: &str) -> io::Result<()> {
        self.inner.remove(path)
    }

    fn chtimes(&self, path: &str, atime: SystemTime, mtime: SystemTime) -> io::Result<()> {
        self.inner.chtimes(path, atime, mtime)
    }

    fn directory_exists(&self, path: &str) -> bool {
        self.add_seen(path);
        self.inner.directory_exists(path)
    }

    fn get_accessible_entries(&self, path: &str) -> Entries {
        self.add_seen(path);
        self.inner.get_accessible_entries(path)
    }

    fn stat(&self, path: &str) -> io::Result<FileInfo> {
        self.add_seen(path);
        self.inner.stat(path)
    }

    fn walk_dir(&self, root: &str, walk_fn: &mut crate::WalkDirFunc<'_>) -> io::Result<()> {
        self.add_seen(root);
        self.inner.walk_dir(
            root,
            &mut |path: &str, entry: DirEntry, err: Option<io::Error>| {
                self.add_seen(path);
                walk_fn(path, entry, err)
            },
        )
    }

    fn realpath(&self, path: &str) -> String {
        self.inner.realpath(path)
    }
}
