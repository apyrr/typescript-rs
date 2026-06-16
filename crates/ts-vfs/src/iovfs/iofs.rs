use std::io;
use std::sync::Arc;
use std::time::SystemTime;

use crate::vfs::{Entries, FileInfo, Fs};

pub trait FsWithSys: Fs {
    fn fsys(&self) -> Arc<dyn Fs + Send + Sync>;
}

pub struct IoFs {
    pub fs: Arc<dyn Fs + Send + Sync>,
    pub use_case_sensitive_file_names: bool,
}

pub fn from(fs: Arc<dyn Fs + Send + Sync>, use_case_sensitive_file_names: bool) -> IoFs {
    IoFs {
        fs,
        use_case_sensitive_file_names,
    }
}

impl Fs for IoFs {
    fn use_case_sensitive_file_names(&self) -> bool {
        self.use_case_sensitive_file_names
    }

    fn file_exists(&self, path: &str) -> bool {
        assert_rooted(path);
        self.fs.file_exists(path)
    }

    fn read_file(&self, path: &str) -> (String, bool) {
        assert_rooted(path);
        self.fs.read_file(path)
    }

    fn write_file(&self, path: &str, data: &str) -> io::Result<()> {
        assert_rooted(path);
        self.fs.write_file(path, data)
    }

    fn append_file(&self, path: &str, data: &str) -> io::Result<()> {
        assert_rooted(path);
        self.fs.append_file(path, data)
    }

    fn remove(&self, path: &str) -> io::Result<()> {
        assert_rooted(path);
        self.fs.remove(path)
    }

    fn chtimes(&self, path: &str, atime: SystemTime, mtime: SystemTime) -> io::Result<()> {
        assert_rooted(path);
        self.fs.chtimes(path, atime, mtime)
    }

    fn directory_exists(&self, path: &str) -> bool {
        assert_rooted(path);
        self.fs.directory_exists(path)
    }

    fn get_accessible_entries(&self, path: &str) -> Entries {
        assert_rooted(path);
        self.fs.get_accessible_entries(path)
    }

    fn stat(&self, path: &str) -> io::Result<FileInfo> {
        assert_rooted(path);
        self.fs.stat(path)
    }

    fn walk_dir(&self, root: &str, walk_fn: &mut crate::WalkDirFunc<'_>) -> io::Result<()> {
        assert_rooted(root);
        self.fs.walk_dir(root, walk_fn)
    }

    fn realpath(&self, path: &str) -> String {
        assert_rooted(path);
        self.fs.realpath(path)
    }
}

impl FsWithSys for IoFs {
    fn fsys(&self) -> Arc<dyn Fs + Send + Sync> {
        self.fs.clone()
    }
}

fn assert_rooted(path: &str) {
    if !(path.starts_with('/')
        || (path.len() >= 3 && path.as_bytes()[1] == b':' && path.as_bytes()[2] == b'/'))
    {
        panic!("vfs: path {path:?} is not absolute");
    }
}
