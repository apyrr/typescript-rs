use std::fs;
use std::path::{Path, PathBuf};

use crate::vfs::Fs;

#[cfg(unix)]
fn mklink(target: &Path, link: &Path, _directory: bool) {
    std::os::unix::fs::symlink(target, link).unwrap();
}

#[cfg(windows)]
fn mklink(target: &Path, link: &Path, directory: bool) {
    if directory {
        std::os::windows::fs::symlink_dir(target, link).unwrap();
    } else {
        std::os::windows::fs::symlink_file(target, link).unwrap();
    }
}

fn setup_symlinks(name: &str) -> (PathBuf, PathBuf) {
    let tmp = std::env::temp_dir().join(format!("tsgo-realpath-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&tmp);
    let target = tmp.join("target");
    let target_file = target.join("file");
    let link = tmp.join("link");
    let link_file = link.join("file");
    fs::create_dir_all(&target).unwrap();
    fs::write(&target_file, "hello").unwrap();
    mklink(&target, &link, true);
    (target_file, link_file)
}

#[test]
fn test_symlink_realpath() {
    let (target_file, link_file) = setup_symlinks("symlink");
    assert_eq!(fs::read_to_string(&link_file).unwrap(), "hello");

    let fs = super::os::fs();
    let target_realpath = fs.realpath(&target_file.to_string_lossy());
    let link_realpath = fs.realpath(&link_file.to_string_lossy());
    assert_eq!(target_realpath, link_realpath);
}

#[test]
fn benchmark_realpath_scenarios_are_represented() {
    let (target_file, link_file) = setup_symlinks("bench");
    let fs = super::os::fs();
    let _ = fs.realpath(&target_file.to_string_lossy());
    let _ = fs.realpath(&link_file.to_string_lossy());

    let mut deep_dir =
        std::env::temp_dir().join(format!("tsgo-realpath-deep-{}", std::process::id()));
    for segment in [
        "project",
        "node_modules",
        "@scope",
        "package",
        "node_modules",
        "dep",
        "lib",
        "dist",
        "esm",
        "internal",
        "utils",
    ] {
        deep_dir = deep_dir.join(segment);
    }
    fs::create_dir_all(&deep_dir).unwrap();
    let deep_file = deep_dir.join("index.js");
    fs::write(&deep_file, "module.exports = {}").unwrap();
    let _ = fs.realpath(&deep_file.to_string_lossy());
}

#[test]
fn test_get_accessible_entries() {
    let tmp = std::env::temp_dir().join(format!("tsgo-realpath-entries-{}", std::process::id()));
    let _ = fs::remove_dir_all(&tmp);
    let target = tmp.join("target");
    let link = tmp.join("link");
    fs::create_dir_all(&target).unwrap();
    fs::create_dir_all(&link).unwrap();

    let target_file1 = target.join("file1");
    let target_file2 = target.join("file2");
    fs::write(&target_file1, "hello").unwrap();
    fs::write(&target_file2, "world").unwrap();

    let target_dir1 = target.join("dir1");
    let target_dir2 = target.join("dir2");
    fs::create_dir_all(&target_dir1).unwrap();
    fs::create_dir_all(&target_dir2).unwrap();

    mklink(&target_file1, &link.join("file1"), false);
    mklink(&target_file2, &link.join("file2"), false);
    mklink(&target_dir1, &link.join("dir1"), true);
    mklink(&target_dir2, &link.join("dir2"), true);

    let fs = super::os::fs();
    let entries = fs.get_accessible_entries(&link.to_string_lossy());
    assert_eq!(entries.directories, vec!["dir1", "dir2"]);
    assert_eq!(entries.files, vec!["file1", "file2"]);
    let symlinks = entries.symlinks.expect("symlinks should be set");
    for name in ["file1", "file2", "dir1", "dir2"] {
        assert!(symlinks.contains(name));
    }

    let entries = fs.get_accessible_entries(&target.to_string_lossy());
    assert_eq!(entries.directories, vec!["dir1", "dir2"]);
    assert_eq!(entries.files, vec!["file1", "file2"]);
    assert!(entries.symlinks.expect("symlinks should be set").is_empty());
}
