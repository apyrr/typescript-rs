use std::io;
use std::sync::Arc;
use std::time::SystemTime;

use super::CachedFs;
use crate::vfs::{DirEntry, Entries, FileInfo, Fs};
use crate::vfsmock::GeneratedFsMock;

fn create_mock_fs() -> Arc<GeneratedFsMock> {
    Arc::new(GeneratedFsMock {
        append_file_func: Some(Arc::new(|_, _| Ok(()))),
        chtimes_func: Some(Arc::new(|_, _, _| Ok(()))),
        directory_exists_func: Some(Arc::new(|path| path == "/some/path")),
        file_exists_func: Some(Arc::new(|path| path == "/some/path/file.txt")),
        get_accessible_entries_func: Some(Arc::new(|_| Entries {
            files: vec!["file.txt".to_owned()],
            directories: vec!["child".to_owned()],
            symlinks: Some(Default::default()),
        })),
        read_file_func: Some(Arc::new(|_| ("hello world".to_owned(), true))),
        realpath_func: Some(Arc::new(|path| format!("{path}/real"))),
        remove_func: Some(Arc::new(|_| Ok(()))),
        stat_func: Some(Arc::new(|path| {
            Ok(FileInfo::directory(
                path.to_owned(),
                Some(SystemTime::UNIX_EPOCH),
            ))
        })),
        use_case_sensitive_file_names_func: Some(Arc::new(|| true)),
        walk_dir_func: Some(Arc::new(|_, _| Ok(()))),
        write_file_func: Some(Arc::new(|_, _| Ok(()))),
        ..GeneratedFsMock::default()
    })
}

#[test]
fn directory_exists_should_cache_while_enabled() {
    let underlying = create_mock_fs();
    let cached = CachedFs::from(underlying.clone());

    assert!(cached.directory_exists("/some/path"));
    assert_eq!(underlying.directory_exists_calls().len(), 1);

    assert!(cached.directory_exists("/some/path"));
    assert_eq!(underlying.directory_exists_calls().len(), 1);

    cached.clear_cache();
    assert!(cached.directory_exists("/some/path"));
    assert_eq!(underlying.directory_exists_calls().len(), 2);

    cached.disable_and_clear_cache();
    assert!(cached.directory_exists("/some/path"));
    assert!(cached.directory_exists("/some/path"));
    assert_eq!(underlying.directory_exists_calls().len(), 4);

    cached.enable();
    assert!(cached.directory_exists("/some/path"));
    assert!(cached.directory_exists("/some/path"));
    assert_eq!(underlying.directory_exists_calls().len(), 5);
}

#[test]
fn stat_should_cache_while_enabled() {
    let underlying = create_mock_fs();
    let cached = CachedFs::from(underlying.clone());

    assert!(cached.stat("/some/path").is_ok());
    assert_eq!(underlying.stat_calls().len(), 1);

    assert!(cached.stat("/some/path").is_ok());
    assert_eq!(underlying.stat_calls().len(), 1);

    assert!(cached.stat("/other/path").is_ok());
    assert_eq!(underlying.stat_calls().len(), 2);
}

#[test]
fn entries_and_realpath_should_cache_while_enabled() {
    let underlying = create_mock_fs();
    let cached = CachedFs::from(underlying.clone());

    assert_eq!(
        cached.get_accessible_entries("/some/path").files,
        vec!["file.txt".to_owned()]
    );
    assert_eq!(
        cached.get_accessible_entries("/some/path").files,
        vec!["file.txt".to_owned()]
    );
    assert_eq!(underlying.get_accessible_entries_calls().len(), 1);

    assert_eq!(cached.realpath("/some/path"), "/some/path/real");
    assert_eq!(cached.realpath("/some/path"), "/some/path/real");
    assert_eq!(underlying.realpath_calls().len(), 1);
}

#[test]
fn direct_methods_should_not_use_cache() {
    let underlying = create_mock_fs();
    let cached = CachedFs::from(underlying.clone());
    let mut walk_fn =
        |_path: &str, _entry: DirEntry, _err: Option<io::Error>| -> io::Result<()> { Ok(()) };

    assert_eq!(cached.read_file("/some/path/file.txt").1, true);
    assert_eq!(cached.read_file("/some/path/file.txt").1, true);
    assert_eq!(underlying.read_file_calls().len(), 2);

    cached
        .write_file("/some/path/file.txt", "new content")
        .unwrap();
    cached.remove("/some/path/file.txt").unwrap();
    cached.walk_dir("/some/path", &mut walk_fn).unwrap();

    assert_eq!(underlying.write_file_calls()[0].data, "new content");
    assert_eq!(underlying.remove_calls().len(), 1);
    assert_eq!(underlying.walk_dir_calls().len(), 1);
}
