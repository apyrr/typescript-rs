use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::sync::Arc;
use std::sync::atomic::{AtomicI64, Ordering};
use std::time::{Duration, SystemTime};

use crate::osvfs;
use crate::{Entries, FileInfo, Fs};

use super::FileWatcher;

struct CountingFs {
    fs: Arc<dyn Fs + Send + Sync>,
    n: AtomicI64,
}

impl CountingFs {
    fn new(fs: Arc<dyn Fs + Send + Sync>) -> Self {
        Self {
            fs,
            n: AtomicI64::new(0),
        }
    }
}

impl Fs for CountingFs {
    fn use_case_sensitive_file_names(&self) -> bool {
        self.fs.use_case_sensitive_file_names()
    }

    fn file_exists(&self, path: &str) -> bool {
        self.fs.file_exists(path)
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
        self.fs.directory_exists(path)
    }

    fn get_accessible_entries(&self, path: &str) -> Entries {
        self.n.fetch_add(1, Ordering::SeqCst);
        self.fs.get_accessible_entries(path)
    }

    fn stat(&self, path: &str) -> io::Result<FileInfo> {
        self.fs.stat(path)
    }

    fn walk_dir(&self, root: &str, walk_fn: &mut crate::WalkDirFunc<'_>) -> io::Result<()> {
        self.fs.walk_dir(root, walk_fn)
    }

    fn realpath(&self, path: &str) -> String {
        self.fs.realpath(path)
    }
}

#[test]
fn test_has_changes_no_redundant_get_accessible_entries() {
    let root = std::env::temp_dir().join(format!(
        "tsgo-vfswatch-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let src = root.join("src");
    let sub = src.join("sub");
    let node_modules = root.join("node_modules");
    fs::create_dir_all(&sub).unwrap();
    fs::create_dir_all(&node_modules).unwrap();
    fs::write(src.join("a.ts"), "const a = 1;").unwrap();
    fs::write(src.join("b.ts"), "const b = 2;").unwrap();
    fs::write(sub.join("c.ts"), "const c = 3;").unwrap();
    fs::write(node_modules.join("x.js"), "").unwrap();
    fs::write(root.join("tsconfig.json"), "{}").unwrap();

    let inner: Arc<dyn Fs + Send + Sync> = Arc::new(osvfs::os::fs());
    let cfs = Arc::new(CountingFs::new(inner));
    let watcher_fs: Arc<dyn Fs + Send + Sync> = cfs.clone();

    let fw = FileWatcher::new(watcher_fs, Duration::from_millis(10), true, || {});
    fw.update_watch_state(
        &[
            src.join("a.ts").to_string_lossy().into_owned(),
            src.join("b.ts").to_string_lossy().into_owned(),
            sub.join("c.ts").to_string_lossy().into_owned(),
            node_modules.to_string_lossy().into_owned(),
            root.join("tsconfig.json").to_string_lossy().into_owned(),
        ],
        &BTreeMap::from([(src.to_string_lossy().into_owned(), true)]),
    );

    cfs.n.store(0, Ordering::SeqCst);

    fw.has_changes_from_watch_state();

    assert_eq!(cfs.n.load(Ordering::SeqCst), 2);

    fs::remove_dir_all(root).unwrap();
}
