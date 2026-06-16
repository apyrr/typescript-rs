use std::io;
use std::time::SystemTime;

use crate::vfs::{Entries, FileInfo, Fs};

pub struct FsMock<F> {
    fs: F,
}

// Wrap wraps a vfs.FS and returns a FSMock which calls it.
pub fn wrap<F: Fs>(fs: F) -> FsMock<F> {
    FsMock { fs }
}

impl<F: Fs> Fs for FsMock<F> {
    fn directory_exists(&self, path: &str) -> bool {
        self.fs.directory_exists(path)
    }

    fn file_exists(&self, path: &str) -> bool {
        self.fs.file_exists(path)
    }

    fn get_accessible_entries(&self, path: &str) -> Entries {
        self.fs.get_accessible_entries(path)
    }

    fn read_file(&self, path: &str) -> (String, bool) {
        self.fs.read_file(path)
    }

    fn realpath(&self, path: &str) -> String {
        self.fs.realpath(path)
    }

    fn remove(&self, path: &str) -> io::Result<()> {
        self.fs.remove(path)
    }

    fn chtimes(&self, path: &str, atime: SystemTime, mtime: SystemTime) -> io::Result<()> {
        self.fs.chtimes(path, atime, mtime)
    }

    fn stat(&self, path: &str) -> io::Result<FileInfo> {
        self.fs.stat(path)
    }

    fn use_case_sensitive_file_names(&self) -> bool {
        self.fs.use_case_sensitive_file_names()
    }

    fn walk_dir(&self, root: &str, walk_fn: &mut crate::WalkDirFunc<'_>) -> io::Result<()> {
        self.fs.walk_dir(root, walk_fn)
    }

    fn write_file(&self, path: &str, data: &str) -> io::Result<()> {
        self.fs.write_file(path, data)
    }

    fn append_file(&self, path: &str, data: &str) -> io::Result<()> {
        self.fs.append_file(path, data)
    }
}
