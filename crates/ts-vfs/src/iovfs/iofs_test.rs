use std::collections::BTreeMap;
use std::sync::Arc;

use crate::vfs::Fs;
use crate::vfstest::from_map;

use super::iofs;

fn test_fs() -> impl Fs {
    from_map(
        BTreeMap::from([
            ("/foo.ts".to_owned(), "hello, world".to_owned()),
            (
                "/dir1/file1.ts".to_owned(),
                "export const foo = 42;".to_owned(),
            ),
            (
                "/dir1/file2.ts".to_owned(),
                "export const foo = 42;".to_owned(),
            ),
            (
                "/dir2/file1.ts".to_owned(),
                "export const foo = 42;".to_owned(),
            ),
        ]),
        true,
    )
}

#[test]
fn test_iofs_read_file() {
    let fs = iofs::from(Arc::new(test_fs()), true);
    assert_eq!(fs.read_file("/foo.ts"), ("hello, world".to_owned(), true));
    assert_eq!(fs.read_file("/does/not/exist.ts"), (String::new(), false));
}

#[test]
#[should_panic(expected = "vfs: path \"bar\" is not absolute")]
fn test_iofs_read_file_unrooted() {
    let fs = iofs::from(Arc::new(test_fs()), true);
    fs.read_file("bar");
}

#[test]
fn test_iofs_file_and_directory_exists() {
    let fs = iofs::from(Arc::new(test_fs()), true);
    assert!(fs.file_exists("/foo.ts"));
    assert!(!fs.file_exists("/bar"));
    assert!(fs.directory_exists("/"));
    assert!(fs.directory_exists("/dir1"));
    assert!(fs.directory_exists("/dir1/"));
    assert!(!fs.directory_exists("/bar"));
}

#[test]
fn test_iofs_get_accessible_entries() {
    let fs = iofs::from(Arc::new(test_fs()), true);
    let entries = fs.get_accessible_entries("/");
    assert_eq!(entries.directories, vec!["dir1", "dir2"]);
    assert_eq!(entries.files, vec!["foo.ts"]);
}

#[test]
fn test_iofs_walk_dir() {
    let fs = iofs::from(Arc::new(test_fs()), true);
    let mut files = Vec::new();
    fs.walk_dir("/", &mut |path, entry, err| {
        if let Some(err) = err {
            return Err(err);
        }
        if !entry.file_type()?.is_dir() {
            files.push(path.to_owned());
        }
        Ok(())
    })
    .unwrap();
    files.sort();
    assert_eq!(
        files,
        vec![
            "/dir1/file1.ts",
            "/dir1/file2.ts",
            "/dir2/file1.ts",
            "/foo.ts"
        ]
    );
}

#[test]
fn test_iofs_realpath_and_case_sensitivity() {
    let fs = iofs::from(Arc::new(test_fs()), true);
    assert_eq!(fs.realpath("/foo.ts"), "/foo.ts");
    assert!(fs.use_case_sensitive_file_names());
}

