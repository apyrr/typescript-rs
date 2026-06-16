use std::collections::BTreeMap;
use std::time::{Duration, SystemTime};

use crate::Fs;

use super::{MapFile, compare_paths_by_parts, from_map, symlink};
use crate::vfs::FileType;

#[test]
fn test_insensitive() {
    let fs = from_map(
        BTreeMap::from([
            ("/foo/bar/baz".to_owned(), "bar".to_owned()),
            ("/foo/bar2/baz2".to_owned(), "bar".to_owned()),
            ("/foo/bar3/baz3".to_owned(), "bar".to_owned()),
        ]),
        false,
    );

    let (sensitive, ok) = fs.read_file("/foo/bar/baz");
    assert!(ok);
    assert_eq!(sensitive, "bar");
    assert_eq!(fs.realpath("/foo/bar/baz"), "/foo/bar/baz");
    assert_eq!(
        fs.get_accessible_entries("/foo").directories,
        vec!["bar", "bar2", "bar3"]
    );
    assert!(fs.stat("/does/not/exist").is_err());

    let (insensitive, ok) = fs.read_file("/Foo/Bar/Baz");
    assert!(ok);
    assert_eq!(insensitive, "bar");
    assert_eq!(fs.realpath("/Foo/Bar/Baz"), "/foo/bar/baz");
    assert_eq!(
        fs.get_accessible_entries("/Foo").directories,
        vec!["bar", "bar2", "bar3"]
    );
}

#[test]
fn test_insensitive_upper() {
    let fs = from_map(
        BTreeMap::from([
            ("/Foo/Bar/Baz".to_owned(), "bar".to_owned()),
            ("/Foo/Bar2/Baz2".to_owned(), "bar".to_owned()),
            ("/Foo/Bar3/Baz3".to_owned(), "bar".to_owned()),
        ]),
        false,
    );

    let (sensitive, ok) = fs.read_file("/foo/bar/baz");
    assert!(ok);
    assert_eq!(sensitive, "bar");
    assert_eq!(
        fs.get_accessible_entries("/foo").directories,
        vec!["Bar", "Bar2", "Bar3"]
    );

    let (insensitive, ok) = fs.read_file("/Foo/Bar/Baz");
    assert!(ok);
    assert_eq!(insensitive, "bar");
    assert_eq!(
        fs.get_accessible_entries("/Foo").directories,
        vec!["Bar", "Bar2", "Bar3"]
    );
}

#[test]
fn test_sensitive() {
    let fs = from_map(
        BTreeMap::from([
            ("/foo/bar/baz".to_owned(), "bar".to_owned()),
            ("/foo/bar2/baz2".to_owned(), "bar".to_owned()),
            ("/foo/bar3/baz3".to_owned(), "bar".to_owned()),
        ]),
        true,
    );

    let (sensitive, ok) = fs.read_file("/foo/bar/baz");
    assert!(ok);
    assert_eq!(sensitive, "bar");
    let (_, ok) = fs.read_file("/Foo/Bar/Baz");
    assert!(!ok);
}

#[test]
#[should_panic(expected = "non-rooted path")]
fn test_from_map_non_rooted() {
    let _ = from_map(
        BTreeMap::from([("string".to_owned(), "hello".to_owned())]),
        false,
    );
}

#[test]
#[should_panic(expected = "non-normalized path")]
fn test_from_map_non_normalized() {
    let _ = from_map(
        BTreeMap::from([("/string/".to_owned(), "hello".to_owned())]),
        false,
    );
}

#[test]
fn test_writable_fs() {
    let fs = from_map(BTreeMap::<String, String>::new(), false);

    fs.write_file("/foo/bar/baz", "hello, world").unwrap();
    let (content, ok) = fs.read_file("/foo/bar/baz");
    assert!(ok);
    assert_eq!(content, "hello, world");

    fs.write_file("/foo/bar/baz", "goodbye, world").unwrap();
    let (content, ok) = fs.read_file("/foo/bar/baz");
    assert!(ok);
    assert_eq!(content, "goodbye, world");

    assert!(fs.write_file("/foo/bar/baz/oops", "nope").is_err());
}

#[test]
fn test_writable_fs_delete() {
    let fs = from_map(BTreeMap::<String, String>::new(), false);

    fs.write_file("/foo/bar/file.ts", "remove").unwrap();
    assert!(fs.file_exists("/foo/bar/file.ts"));
    fs.remove("/foo/bar/file.ts").unwrap();
    assert!(!fs.file_exists("/foo/bar/file.ts"));

    fs.write_file("/foo/bar/test/remove2.ts", "remove2")
        .unwrap();
    assert!(fs.directory_exists("/foo/bar/test"));
    fs.remove("/foo/bar/test").unwrap();
    assert!(!fs.file_exists("/foo/bar/test/remove2.ts"));
    assert!(!fs.directory_exists("/foo/bar/test"));

    fs.remove("/foo/bar/test").unwrap();
    fs.remove("/foo/bar/file.ts").unwrap();

    fs.write_file("/foo/barbar", "remove2").unwrap();
    fs.remove("/foo/bar").unwrap();
    assert!(fs.file_exists("/foo/barbar"));
}

#[test]
fn test_vfstest_map_fs() {
    let fs = from_map(
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
        false,
    );

    let (content, ok) = fs.read_file("/foo.ts");
    assert!(ok);
    assert_eq!(content, "hello, world");
    let (missing, ok) = fs.read_file("/does/not/exist.ts");
    assert!(!ok);
    assert_eq!(missing, "");
    assert_eq!(fs.realpath("/Foo.ts"), "/foo.ts");
    assert_eq!(fs.realpath("/does/not/exist.ts"), "/does/not/exist.ts");
    assert!(!fs.use_case_sensitive_file_names());
}

#[test]
fn test_vfstest_map_fs_windows() {
    let fs = from_map(
        BTreeMap::from([
            ("c:/foo.ts".to_owned(), "hello, world".to_owned()),
            (
                "c:/dir1/file1.ts".to_owned(),
                "export const foo = 42;".to_owned(),
            ),
            (
                "c:/dir1/file2.ts".to_owned(),
                "export const foo = 42;".to_owned(),
            ),
            (
                "c:/dir2/file1.ts".to_owned(),
                "export const foo = 42;".to_owned(),
            ),
        ]),
        false,
    );

    let (content, ok) = fs.read_file("c:/foo.ts");
    assert!(ok);
    assert_eq!(content, "hello, world");
    assert_eq!(fs.realpath("c:/Foo.ts"), "c:/foo.ts");
    assert_eq!(fs.realpath("c:/does/not/exist.ts"), "c:/does/not/exist.ts");
}

#[test]
fn test_bom() {
    let expected = "hello, world";
    let fs = from_map(BTreeMap::<String, MapFile>::new(), true);
    fs.set(
        "/utf8.ts",
        MapFile {
            data: format!("\u{feff}{expected}").into_bytes().into(),
            text: None,
            mode: FileType::file(),
            mod_time: SystemTime::UNIX_EPOCH,
            realpath: "/utf8.ts".to_owned(),
        },
    );
    let mut utf16le = vec![0xFF, 0xFE];
    for unit in expected.encode_utf16() {
        utf16le.extend_from_slice(&unit.to_le_bytes());
    }
    fs.set(
        "/utf16le.ts",
        MapFile {
            data: utf16le.into(),
            text: None,
            mode: FileType::file(),
            mod_time: SystemTime::UNIX_EPOCH,
            realpath: "/utf16le.ts".to_owned(),
        },
    );

    assert_eq!(fs.read_file("/utf8.ts"), (expected.to_owned(), true));
    assert_eq!(fs.read_file("/utf16le.ts"), (expected.to_owned(), true));
}

#[test]
fn test_symlink() {
    let fs = from_map(
        BTreeMap::from([
            ("/foo.ts".to_owned(), "hello, world".to_owned()),
            ("/some/dir/file.ts".to_owned(), "hello, world".to_owned()),
            (
                "/d/existing.ts".to_owned(),
                "this is existing.ts".to_owned(),
            ),
        ]),
        false,
    );
    fs.set("/symlink.ts", symlink("/foo.ts"));
    fs.set("/some/dirlink", symlink("/some/dir"));
    fs.set("/a", symlink("/b"));
    fs.set("/b", symlink("/c"));
    fs.set("/c", symlink("/d"));

    assert_eq!(
        fs.read_file("/symlink.ts"),
        ("hello, world".to_owned(), true)
    );
    assert_eq!(
        fs.read_file("/some/dirlink/file.ts"),
        ("hello, world".to_owned(), true)
    );
    assert_eq!(
        fs.read_file("/a/existing.ts"),
        ("this is existing.ts".to_owned(), true)
    );
    assert_eq!(fs.realpath("/symlink.ts"), "/foo.ts");
    assert_eq!(fs.realpath("/some/dirlink"), "/some/dir");
    assert_eq!(fs.realpath("/some/dirlink/file.ts"), "/some/dir/file.ts");
    assert!(fs.file_exists("/a/existing.ts"));
    assert!(fs.directory_exists("/a"));
}

#[test]
fn test_writable_fs_symlink() {
    let fs = from_map(
        BTreeMap::from([
            ("/some/dir/other.ts".to_owned(), "NOTHING".to_owned()),
            ("/d/existing.ts".to_owned(), "hello, world".to_owned()),
        ]),
        false,
    );
    fs.set("/other.ts", symlink("/some/dir/other.ts"));
    fs.set("/some/dirlink", symlink("/some/dir"));
    fs.set("/a", symlink("/b"));
    fs.set("/b", symlink("/c"));
    fs.set("/c", symlink("/d"));

    fs.write_file("/some/dirlink/file.ts", "hello, world")
        .unwrap();
    assert_eq!(
        fs.read_file("/some/dir/file.ts"),
        ("hello, world".to_owned(), true)
    );
    fs.write_file("/other.ts", "hello, world").unwrap();
    assert_eq!(
        fs.read_file("/some/dir/other.ts"),
        ("hello, world".to_owned(), true)
    );
    fs.write_file("/a/foo/bar/new.ts", "this is new.ts")
        .unwrap();
    assert_eq!(
        fs.read_file("/d/foo/bar/new.ts"),
        ("this is new.ts".to_owned(), true)
    );
}

#[test]
fn test_ch_times_and_stat() {
    let fs = from_map(
        BTreeMap::from([("/foo.ts".to_owned(), "hello".to_owned())]),
        true,
    );
    let mtime = SystemTime::UNIX_EPOCH + Duration::from_secs(1234);
    fs.chtimes("/foo.ts", mtime, mtime).unwrap();
    let info = fs.stat("/foo.ts").unwrap();
    assert!(info.is_file());
    assert_eq!(info.name(), "foo.ts");
    assert_eq!(info.modified().unwrap(), mtime);
}

#[test]
fn test_walk_dir() {
    let fs = from_map(
        BTreeMap::from([
            ("/src/a.ts".to_owned(), "a".to_owned()),
            ("/src/sub/b.ts".to_owned(), "b".to_owned()),
        ]),
        true,
    );
    let mut seen = Vec::new();
    fs.walk_dir("/src", &mut |path, _entry, err| {
        assert!(err.is_none());
        seen.push(path.to_owned());
        Ok(())
    })
    .unwrap();
    seen.sort();
    assert_eq!(seen, vec!["/src/a.ts", "/src/sub", "/src/sub/b.ts"]);
}

#[test]
fn test_compare_paths_by_parts() {
    let mut values = vec!["/a/b", "/a", "/a/a", "/b"];
    values.sort_by(|a, b| compare_paths_by_parts(a, b));
    assert_eq!(values, vec!["/a", "/a/a", "/a/b", "/b"]);
}
